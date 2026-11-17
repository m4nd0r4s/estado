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

## Follow-up: the report arithmetic of `send_batch`

Date: 2026-11-13. A CI x86_64 run after the decision above (run on 61586ea, quick mode) gave baseline 1.236 ms, hand-written `send_into` 1.313 (1.06x) and `send_batch` 1.742 (1.41x), macro `send_into` 1.336 (1.08x) and `send_batch` 2.010 (1.63x). The target of 1.25x was not met: `send_batch` cost about 0.7 ns per byte more than a loop of `send_into`, which should be impossible for a loop that does strictly less work. Two causes, one in the benchmark and one in the core.

### 1. The two variants did not do the same work

`run_estado_send_into` and `run_macro_send_into` threw every report away (`let _ = machine.send_into(..)`), so the optimizer deleted the report arithmetic from them, while `send_batch` returns a report that goes through `black_box`, so its arithmetic has to run. The comparison favoured `send_into`. What is right: both sides give a caller what that caller reads. A caller of `send_batch` reads the totals; a caller of `send_into` per byte that wants the same totals folds the reports and checks the error. The `send_into` variants now do exactly that (`send_each` in `bm_01_http_parser.rs`: counters summed into wide totals, `handled` and `ignored` of the last report, the error checked, the whole `Totals` through `black_box`). A loop that ignores the reports is not a scenario worth a variant: it measures a `Machine` whose report does not exist. The change makes `send_into` slower by 3-10% on aarch64 (macro 1.55 to 1.59-1.60 ms, hand-written 1.59 to 1.74 ms), so the ratios of `send_into` quoted before this follow-up are too low by that much.

### 2. The loop built a `Report` per event and added it to the total

The loop declared a `Report`, let the step fill it (`count`, `ignored`, the livelock bound), and then `BatchTotal::add` widened its fields into the running total. The asm on x86_64 shows what that is: every handler path computes its contribution in registers (`xor`, `mov $1`), joins, and a few `add`s fold it, in a loop where the state, the iterator and the totals already compete for registers. `microstep`, `drain` and `step_live` are now generic over a small private trait, `Tally` (`machine/tally.rs`). `Report` implements it as before, for `send_into`, `start` and the tasks; `BatchTotal` implements it with wide counters, so the batch loop counts in place (`incq` of the microsteps and, on a transition, of the transitions) and never builds a `Report`. The livelock bound is per event, so `BatchTotal::begin_event` records where the event's microsteps started and `microsteps_of_step` returns the difference; the error carries the same count as a single `send_into` (new test `the_livelock_bound_counts_one_event_not_the_batch`, next to the existing ones in `tests/send_batch.rs`, which pin the rest: sums, last flags, first error, panic, clamping). The guard stays per event, for the reason given above. The step code moved from `machine/mod.rs` to `machine/step.rs` in the same commit, which was at 591 lines; `mod.rs` is 427 now.

Upper bound measured before writing it: with the arithmetic removed from the loop altogether (counting only `microsteps += 1`) the macro `send_batch` on x86_64 under Rosetta went from 3.29 to 2.55 ms, so the arithmetic was most of the gap, and the layout-independent part of it is what the change removes.

### Measurements

Release, 1 MB of requests, criterion `--quick`, **7 runs alternating between the two binaries** (old, new) so that system load hits both alike; the machine was busy (load average 14-18), so the table gives medians of 7 runs. Old is the tree with the fair `send_into` variants and the previous core; new is the tree of this follow-up. Both are quoted from the same bench source.

Apple M-series (aarch64, native):

| Measurement | Old | New |
| --- | --- | --- |
| baseline | 0.988 ms | 0.981 ms |
| hand-written `send_batch` | 1.651 ms (1.67x) | 1.651 ms (1.68x) |
| hand-written `send_into` (reports read) | 1.741 ms (1.76x) | 1.748 ms (1.78x) |
| macro `send_batch` | 1.476 ms (1.49x) | 1.436 ms (1.46x) |
| macro `send_into` (reports read) | 1.603 ms (1.62x) | 1.593 ms (1.62x) |
| `microstep` direct, hand-written / macro | 1.232 / 1.248 ms | 1.291 / 1.267 ms |
| BM-02, estado side | 1.903 ms | 1.915 ms |

x86_64 under Rosetta 2:

| Measurement | Old | New |
| --- | --- | --- |
| baseline | 2.639 ms | 2.417 ms |
| hand-written `send_batch` | 2.413 ms | 2.271 ms |
| hand-written `send_into` (reports read) | 2.461 ms | 2.479 ms |
| macro `send_batch` | 3.297 ms | 2.613 ms |
| macro `send_into` (reports read) | 2.984 ms | 2.979 ms |
| BM-02, estado side | 2.537 ms | 2.546 ms |

Reading:

- `send_batch` is now faster than a loop of `send_into` that reads the same reports, on both architectures and for both machines (macro: 1.44 against 1.59 ms on aarch64, 2.61 against 2.98 ms under Rosetta). Before, the macro `send_batch` was 11% slower than the loop under Rosetta (3.30 against 2.98 ms) and 8% faster on aarch64.
- The macro `send_batch` under Rosetta improves by 21%, on aarch64 by 3%; the hand-written one by 6% and 0%. `send_into` per byte and BM-02 do not move on either architecture, as asked.
- Rosetta does not give the baseline or the other variants a stable absolute number between binaries: the baseline alone went from 2.0 to 2.7 ms across builds of identical source in the runs of this work, and the `send_into` loop of the hand-written machine from 2.3 to 2.9 ms. Only the ratios inside one binary mean something, and Rosetta has been optimistic about `send_batch` compared with CI (before this change it showed the macro `send_batch` at about 1.25x where CI showed 1.63x). The CI x86_64 run decides.

### Tried and dropped

- The earlier drops (guard removal, done-check removal, `#[inline(always)]` on `version_step`, the `Edge::apply` prologue) stay dropped.

### Decision

1. Keep the in-place counting (`Tally`, `BatchTotal` counting directly) and the split of `machine/step.rs`.
2. Keep the fair `send_into` variants as the comparison to `send_batch`.
3. The target stays at **1.25x of the hand-written parser on the CI x86_64 runner** for the macro `send_batch`. The CI ratio of the macro `send_into`, which does the same per-byte work as the new loop with a slot test more, was 1.08x before this follow-up and the fair loop is a little slower, so the new `send_batch` should land around 1.1x-1.2x. Tightening the target to 1.10x is worth proposing only after a CI run shows it; tightening on the strength of Rosetta numbers would repeat the mistake of the first revision. If that run shows the macro `send_batch` at or under 1.10x, errata 18 gets a line saying so and the bead can close.

## Generated code: why the macro machine was 30% slower than the hand-written one

Date: 2026-11-17. The CI x86_64 run after the `Tally` change (quick mode, run on main after 427b42b, baseline 2.849 ms) showed the core was no longer the bottleneck: hand-written `send_batch` 2.925 ms (1.03x), macro `send_batch` 3.678 (1.29x), macro `send_into` 3.403 (1.19x), and `microstep` called directly 2.425 (hand-written) against 3.170 (macro). Same parser, same grammar functions, same `Machine`: the difference is the code `#[machine]` generates (`examples/a_http_parser_macro`) against the hand-written `Spec` (`examples/a_http_parser`). The two were compared by `cargo expand`, by the asm of `run_microstep` for both on x86_64 and aarch64, and by the inliner's own report (`-C llvm-args=-pass-remarks-missed=inline`).

### Root cause

The dispatch is the same shape on both sides (one match on the root discriminant, one on the substate, the handler, `resolve`). The differences are in the edges:

1. **One `apply` arm per `goto!`, and the inliner priced `apply` just over its threshold.** A handler with five `goto!`s has five variants (ADR 0001) and, until now, five arms in `Edge::apply`, four of them the same code (`goto!(Error(..))` in different branches of `on_value`). The hand-written parser has two variants for that handler. The base asm on x86_64 has one out-of-line call to `ValueEdges::apply` in the hot loop, and LLVM's report (measured with the arms unmerged and the edges already small) says why: "too costly to inline (cost=340, threshold=325)" in `run_microstep`, 410 against 45 in the cold paths. A call that is not inlined takes the address of the slot: the slot can no longer be split into scalars, so the whole state (discriminant, substate, fields) lives in memory for the entire loop, and the jump table addresses are reloaded after every call. The hand-written machine inlines every `apply` and its state is in registers. (The same function was not checked on aarch64; the gap there was 8%, so whatever the inliner does there costs less.)
2. **The edges carried whole state values.** `goto!(Error(reason))` built a `HttpParser` (32 bytes) in the handler, put it in the edge, and `apply` copied it into the slot; the hand-written edge carries the `ParseError` (1 byte) and `apply` builds the state. That is a copy of the largest variant per transition, a larger `Next` returned from every handler, and one more thing the optimizer has to see through. It is also what made the arms of point 1 expensive.

Everything else the brief asked about is already as cheap as the hand-written code: the nested niche-packed enums are the same layout on both sides (the macro flattens the shared fields into the compound struct, the hand-written one keeps them in a nested struct; no difference in the asm), the dispatch decodes once per event, `Next` is constructed the same way, bubbling is the same two `resolve` calls, the exit and entry sequences of this machine are empty and fold away, `Slot::take`/`put` are not used by simple-form edges, tracking of the active configuration is off, and there is no completion or `always` flag code in a machine without them (`PendingFlags` is a constant zero). The macro also emits the `Next::Defer` arm that the hand-written `resolve` leaves `unreachable!`; it is a cold arm and costs nothing in the loop.

### Changes

- `generate/edges.rs`: the arms of `Edge::apply` that do the same (same bindings, same types, same code) are merged into one or-pattern (`ToError(a) | ToError2(a) | ToError3(a) => ..`). The variants stay one per `goto!`, so the graph and its event hints are unchanged; only the code of `apply` stops growing with the number of `goto!`s.
- `handlers/goto_resolve.rs`, `goto_value.rs`, `construct_context.rs`: a plain `goto!` (data only, no `#[initial(with)]` below its target) now uses the form that errata 15 created for initializers that need the `Cx`: the handler evaluates the data the user wrote, the edge carries it, `apply` builds the value after the exits. The edge is as small as the data, as in the hand-written parser. A target whose initial chain calls a zero-parameter `#[initial(with)]` keeps building the value in the handler (ADR 0016 says that form builds where the `goto!` is written), and so do history targets. The test `crates/estado-macros/tests/edge_shape.rs` pins the edge size (2 bytes where it was 48), the data of merged variants, and the order of the zero-parameter initializer before the exits.
- No `#[inline(always)]` was added, so spec 6.5 and errata 10 stand unchanged. `#[inline]` on `Edge::apply` and on the `exit_*`/`enter_*` sequence functions was tried and dropped: identical asm and times once the arms were merged (the plain hint matters only for a handler with many different `goto!`s).

### Ablation (release, aarch64, min of 7 runs of macro `send_batch` / `send_into`)

| Tree | `send_batch` | `send_into` |
| --- | --- | --- |
| before (427b42b) | 1.419 ms | 2.5-3.0 ms (loaded machine) |
| arms merged, edges still carry values | 1.380 ms | |
| arms merged and edges carry data (committed) | 1.245 ms | |

Merging alone moves the aarch64 numbers by 3%; the small edges are most of it there. On x86_64 the asm of `run_microstep` for the macro machine loses its call to `apply` with the merge alone (760 lines against 673 with both), but under Rosetta the merge alone did not help (macro `send_into` 3.09 ms against 2.21 ms with both, base 3.05 ms): a call that is inlined but whose arms still copy 32-byte values is not what the hand-written code does.

### Measurements

Release, 1 MB of requests, criterion `--quick`, **9 runs alternating the old and the new binary** (the machine was busy, load average 12-17, so the table gives medians and ratios inside one binary run); Rosetta absolute numbers drift 10-25% between builds of identical source and its hand-written `microstep` is slower than the macro one, which CI does not show, so the Rosetta columns are a direction, not a prediction of CI.

Apple M-series (aarch64, native), baseline 0.99 ms:

| Measurement | Before | After |
| --- | --- | --- |
| hand-written `send_batch` | 1.689 ms (1.71x) | 1.673 ms (1.69x) |
| macro `send_batch` | 1.447 ms (1.46x) | 1.124 ms (1.13x) |
| macro `send_into` (reports read) | 1.591 ms (1.61x) | 1.335 ms (1.35x) |
| macro `microstep` direct | 1.247 ms (1.26x) | 1.22-1.33 ms (layout noise between binaries of the same source) |
| BM-02, estado side | 1.903 ms | 1.903 ms |

x86_64 under Rosetta 2, baseline 2.4 ms:

| Measurement | Before | After |
| --- | --- | --- |
| macro `send_batch` | 2.651 ms (1.10x) | 2.131 ms (0.89x) |
| macro `send_into` (reports read) | 2.924 ms (1.21x) | 1.972 ms (0.82x) |
| hand-written `send_batch` (same source, other binary) | 2.326 ms | 2.648 ms |

The macro `send_batch` is now faster than the hand-written estado `send_batch` on aarch64 (1.12 against 1.67 ms), and the macro `send_into` gains 16-33% on both architectures. BM-02 is a core-only benchmark and did not move. BM-05 (`scripts/bm05.sh`): 1.10 s for 200 states and 600 edges (1.07 s before), under the 3 s target.

### What is left

On aarch64 the macro machine called through `microstep` directly is still 1.2-1.3x of the baseline, like the hand-written one (1.20x): that part is the representation of ADR 0017 above, not the generator. Whether the CI x86_64 ratio lands at or under 1.10x (the figure at which errata 18 can be withdrawn and the bead closed) is for the next CI run; the target of 1.25x stays until then.
