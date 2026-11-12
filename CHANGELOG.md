# Changelog

## M3 — the macro (in progress: BM-01 open)

- `#[machine]` on an inline module (ADR 0010) generates the code M2 wrote by hand (ADR 0011, 0012): hierarchy, constructors and views, history, `#[terminal]` with `#[on_done]`, `#[after]` and `#[invoke]`, `cx.raise` and `defer!()`, parallel regions, `#[always]`, `#[unhandled]` overrides.
- Scenarios A, B, D and I run on the macro with the same test cases as the hand-written versions.
- The `estado` facade works on its own (`proc-macro-crate` path resolution); the README quick start is a doctest.
- Diagnostics: 30 error codes and warnings W0001-W0005, each with a page in `docs/errors/` and a trybuild snapshot (AC-17).
- Core fixes from the reviews: `in_state` in `#[always]` and initial entries, implicit start drains, any start fault poisons, one livelock budget per send.
- BM-05: about 0.9 s for a 200-state, 600-edge machine (target 3 s). BM-01 still above target (CI: macro `send_into` 1.17x, `send_batch` 1.48x), tracked in bead estado-99i.3.16.
- Four review rounds; decisions in ADR 0013 (with amendments 1-20); errata 7-15.

## M2 — hand-written core

- `estado-core`: `Spec`, `Machine` with the run-to-completion algorithm (queues, defer, policies, livelock, poisoning through `Slot`), `Cx`, effects and sinks, `fire`/`complete` with epochs and stale keys, `Driver`/`drive`. `no_std` without alloc, no dependencies.
- `estado-test`: `Harness` with a virtual clock, `assert_state!`.
- Acceptance tests AC-01 to AC-10, AC-15, AC-16, AC-18 on hand-written machines.
- Scenarios A (HTTP/1 parser), B (connection), D (game character) and I (media player) written by hand in `estado-examples`.
- BM-02 at parity with hand-written `enum` + `match`. BM-01 at 1.6-1.7x, carried to M3 (ADR 0009).
- ADRs 0004-0009; spec errata 5-6.

## M1 — risk prototypes

- Typed edges `Next<M, impl Edge<M>>` work on stable; plan B for R1 is not needed (ADR 0001).
- The poisoned state lives in a core-owned `Slot<M>`, no hidden variant (ADR 0002, closes 13.1).
- Warnings W0001-W0010 use generated `#[deprecated]` items with the user's span (ADR 0003).
- Spec errata 1-4: AC-15 restated against `(M, Ctx)`, started bit, `Slot::Poisoned`, `Edge::apply`.

## M0 — skeleton

- Workspace with the nine crates from spec section 2, all empty.
- CI: rustfmt, clippy `-D warnings`, tests in debug and release (AC-19), MSRV 1.85, `estado-core` on `thumbv7em-none-eabihf` without alloc (AC-16), Miri (AC-18).
- `docs/` layout: spec, errata, ADRs, error pages.
