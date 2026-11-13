# 0017. BM-01: a dedicated batch loop, and a revised target for a one-event-per-byte machine

Status: accepted
Date: 2026-11-13
Spec: 10.3 BM-01, 6.5; ADR 0009, ADR 0011, ADR 0012; spec errata 10, 13 and 18

## Context

ADR 0009 carried BM-01 (the scenario A HTTP/1 parser within 5% of a hand-written parser) into M3 and asked for a re-measurement against the macro-generated machine and, if it still missed, an errata entry with numbers. CI (x86_64, quick mode, after the M3 fix rounds) measured: baseline 2.787 ms, hand-written estado `send_into` 2.596 (0.93x), `send_batch` 3.537 (1.27x), macro `send_into` 3.273 (1.17x), macro `send_batch` 4.123 (1.48x). BM-02 was at parity. Two things stood out: `send_batch` was slower than a loop of `send_into`, and the macro machine was slower than the hand-written one.

Bead estado-99i.3.16 asked to close the gap. This ADR records what was done, what the measurements say, and the target that follows.

## What was done

1. **`send_batch` has its own loop** (`crates/estado-core/src/machine/batch.rs`). The slot is tested once before the loop (an unstarted machine starts, a poisoned one refuses, a finished one ignores, all through the unchanged slow path), then each event runs `step_live` (the done check, the microstep and the drain, shared with `send_live_from`) with its own poison guard and a local report that is added to wide counters, clamped once at the end. There is no per-event `Result<Report, _>` and no per-event slot test. The guard stays per event on purpose: a guard around the whole loop would also poison when the caller's iterator panics in `next()`, which a loop of `send_into` does not. The merge of the reports is the same as before. New tests, `crates/estado-core/tests/send_batch.rs`, pin it against a loop of `send_into` (counters, empty batch, unstarted machine, first error, panic, livelock, clamping).
2. **BM-01 reads the report.** The `send_batch` result now goes through `black_box` like the context. The old `let _ =` let the optimizer delete the report arithmetic; a caller does read it, so the benchmark must pay for it. This makes the estado side slower or equal, never faster, and the baseline is unchanged (the open question of giving the baseline an equivalent per-event result stays unanswered: nothing in this ADR depends on it).

## Measurements

Local, release, 1 MB of requests, criterion `--quick`, median of 5 runs, `black_box`ed report on both sides of the comparison.

Apple M-series (aarch64, native), baseline 0.97-0.98 ms:

| Measurement | Before | After |
| --- | --- | --- |
| hand-written estado, `send_batch` | 1.76 ms (1.79x) | 1.62 ms (1.67x) |
| macro estado, `send_batch` | 1.65 ms (1.68x) | 1.46 ms (1.50x) |
| hand-written estado, `send_into` per byte | 1.59 ms (1.62x) | 1.56 ms (1.61x) |
| macro estado, `send_into` per byte | 1.52 ms (1.54x) | 1.51 ms (1.55x) |
| hand-written estado, `microstep` direct | 1.21 ms (1.23x) | 1.20 ms (1.23x) |
| macro estado, `microstep` direct | 1.26 ms (1.28x) | 1.23 ms (1.27x) |
| BM-02, estado side | 1.92 ms | 1.87 ms |

x86_64 under Rosetta 2 (the same LLVM x86 code, translated; it reproduces the shape of the CI numbers but not their absolute values, and its baseline varies from 2.3 to 2.9 ms between runs, so compare milliseconds of the estado side):

| Measurement | Before | After |
| --- | --- | --- |
| hand-written estado, `send_batch` | 2.63 ms | 2.74 ms |
| macro estado, `send_batch` | 3.83 ms | 2.96 ms |
| macro estado, `send_into` per byte | 2.61 ms | 2.25 ms |
| BM-02, estado against hand-written | 1.01x | 1.00x |

BM-02 does not move on either architecture: the estado side stays at 1.87-1.92 ms (aarch64) and 2.55-2.59 ms (x86_64 under Rosetta). The hand-written side of BM-02 on aarch64 flipped between 1.93 ms and 3.33 ms across builds of identical source (layout again), so its ratio is not quoted; on x86_64 it is 2.55 ms in both. BM-05 (`scripts/bm05.sh`) was 1.07 s for 200 states and 600 edges; the change does not touch the macro.

## Tried and dropped

- Removing the poison guard from the loop: no change on aarch64. The guard is not the cost.
- Removing the done check from the loop: the macro `send_batch` got slower (1.74 ms) and the macro `send_into` faster (1.34 ms) in the same run. Equal code moved by 20% in both directions. Local numbers at this level are decided by register allocation and layout, not by the work done, which is what the notes of the bead found for BM-02 as well. Not kept.
- `#[inline(always)]` on `grammar::version_step` (shared by both sides): macro `send_batch` 1.46 to 1.36 ms on aarch64, 2.96 to 3.19 ms under Rosetta. Not consistent across architectures, so not kept.
- Hoisting the shared root-to-LCA prologue of the `Edge::apply` arms (the BM-05 finding): not done. After inlining the arms are the same code, so it cannot help BM-01, and BM-05 is at a third of its 3 s target. The `take`/`put` of the root for sibling transitions was already gone: simple-form edges write below the root in place (ADR 0011), as the asm of the generated `apply` shows.
- No new `#[inline(always)]`: the existing ones (errata 10) stay; nothing else was needed, so no new errata entry about inlining.

## Why a gap remains on aarch64

The loops are as small as they can be: the asm of `run_macro` has no calls except `version_step` (as the baseline has), no report arithmetic, no queue code (`NoRuntime`). What is left is the representation. The baseline is one flat `enum` of 10 variants: one jump table on a discriminant that stays in a register. The machine is a root enum whose compound variants hold a second enum (`RequestLineSub`, `HeadersSub`), niche-packed into one discriminant that every event decodes with a compare chain (`sub`, `cmp`, `csinc`) before the handler's own test, and the `Slot` it sits in is read from and written to memory around every event (taking the guard out did not change that: no difference in time). `microstep` called directly, without `Machine`, already costs 1.23-1.27x; `send_batch` adds about 0.2 ms on top (the done check, the `Live` test of `live_mut`, the outcome decode). Closing that means a different state representation (flat discriminants), which is the design of the library, not a defect of the generator.

## Decision

1. Keep both changes: the dedicated `send_batch` loop and the report read in BM-01.
2. The 5% target of spec 10.3 is kept as the goal for BM-02-style machines (flat, one `enum`) and met there. For a machine of one event per byte with a compound hierarchy, which is the worst case of the per-event cost of the machine layer, the target of BM-01 is revised to **1.25x of the hand-written parser on the CI x86_64 runner** (macro-generated machine, `send_batch`; ADR 0009 allowed "about 2x"; the measured CI ratio of macro `send_into` before this change, 1.17x, shows 1.25x is reachable and 5% is not a property the representation has on every CPU). On Apple Silicon the ratio is about 1.5x and is reported, not asserted.
3. The BM-01 test does not assert the ratio, as before: timing is not a test. CI keeps printing the numbers.
4. If the CI numbers after this change show the macro `send_batch` within 5%, this ADR is superseded and errata 18 is closed with the original target.

## Consequences

- `docs/spec.md` 10.3 is not edited; errata 18 carries the revision (the spec is revised by its owner).
- The bead estado-99i.3.16 stays open until a CI run gives the numbers after this change; local numbers are a proxy (aarch64 native and x86_64 under Rosetta 2), and the CI x86_64 figure decides.
- The landing page figure (BM-01, 1.17x) is stale until that run.
