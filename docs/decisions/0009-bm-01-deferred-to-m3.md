# 0009. BM-01 is met with the macro, not in M2

Status: accepted
Date: 2026-11-12
Spec: 1.1 principle 2, 10.3 BM-01, 12 (M2 definition of done)

## Context

M2's definition of done includes BM-01: the scenario A HTTP/1 parser within 5% of an equivalent hand-written parser. Measured on CI (x86_64, 1 MB of requests): hand-written 1.49 ms, estado `send_batch` 2.51 ms (1.69x), `send_into` per byte 2.35 ms (1.58x). BM-02 (flat ping-pong) is at parity on the same runner (2.727 ms vs 2.732 ms).

Experiments (bead estado-99i.3.16):
- Inlining the send loop in the core took `send_batch` from 4.5x to 1.75x and is kept.
- Restructuring the loop further helped the parser (1.30x per byte) but cost BM-02 1.8x, so it was reverted: the machine layer is sensitive to inlining and code layout, and tuning one benchmark degrades the other.
- Calling the generated `microstep` directly, without `Machine`, gives 1.23x. The hand-written scenario code takes and puts the whole root for every transition, while the baseline mutates in place.

## Decision

Start M3 with BM-01 open. Bead estado-99i.3.16 moves to M3: the remaining gain is in the shape of generated code (in-place writes for transitions whose LCA is below the root, `#[inline(always)]` on `microstep` and the per-compound dispatch) and in a batch loop that keeps one running report. The macro generates that code, and BM-01 is re-measured on CI against it.

## Consequences

- M2 closes with every criterion met except BM-01, which becomes part of M3's definition of done.
- If the macro-generated parser still misses 5%, the target for a one-event-per-byte machine is revisited in an errata entry, with numbers.
- Spec 6.5 says generated code does not force `#[inline(always)]`; if the macro needs it on `microstep`, that becomes an errata entry.
