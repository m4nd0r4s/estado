# Changelog

## M4 — graph and testing (in progress)

- `estado-graph` exports a machine's `GRAPH` to Mermaid, DOT, SCXML and JSON (schema in `docs/graph.schema.json`), with `assert_snapshot!` and `ESTADO_BLESS=1` (ADR 0018). Mermaid and DOT output is checked by rendering with mermaid-cli and Graphviz.
- Snapshots of scenarios A, B, D and I in every format under `docs/graphs/`.
- `Graph::initial` records the `#[initial]` of the root enum; the exports use it and the JSON gained `initial` (`JSON_VERSION` 2, errata 19 resolved).
- Analyses of spec 8.3 (ADR 0019): `GraphAnalysis` with `reachable`, `dead_ends`, `edge_cover` (the minimum number of paths) and `shortest_path`, and `Graph::worst_case_actions` as a `const fn` of `estado-core`; `#[machine]` generates `Root::WORST_CASE_ACTIONS`.
- W0001 no longer reports the other regions of a parallel state entered by an explicit path.

## M3 — the macro

- `#[machine]` on an inline module (ADR 0010) generates the code M2 wrote by hand (ADR 0011, 0012): hierarchy, constructors and views, history, `#[terminal]` with `#[on_done]`, `#[after]` and `#[invoke]`, `cx.raise` and `defer!()`, parallel regions, `#[always]`, `#[unhandled]` overrides.
- Scenarios A, B, D and I run on the macro with the same test cases as the hand-written versions.
- The `estado` facade works on its own (`proc-macro-crate` path resolution); the README quick start is a doctest.
- Diagnostics: 30 error codes and warnings W0001-W0005, each with a page in `docs/errors/` and a trybuild snapshot (AC-17).
- Core fixes from the reviews: `in_state` in `#[always]` and initial entries, implicit start drains, any start fault poisons, one livelock budget per send.
- BM-05: about 1.1 s for a 200-state, 600-edge machine (target 3 s).
- `send_batch` has a dedicated loop over a live machine: the macro machine drops about 12% on Apple Silicon and 23% under Rosetta (x86_64), BM-02 unchanged. BM-01 target revised to 1.25x for a one-event-per-byte machine (ADR 0017, errata 18).
- `send_batch` counts into one running total instead of a `Report` per event; the generated `Edge::apply` merges identical arms and a plain `goto!` carries only its data. BM-01 on CI x86_64: macro `send_batch` 1.19x the hand-written parser (from 1.48x); BM-02 at parity.
- Fixes from the docs review: more than 32 active states is a compile error (E0018); panics in `#[after]` event expressions, effect overflows and mid-step defer overflows poison (ADR 0014); `goto_dyn!`, `edges!` and transition macros in role-less helpers report E0090; state id constants end in `_ID`; `cx.in_state` without tracking panics (ADR 0015); `#[initial(with = f)]` can take the `Cx` and the parent's fields (ADR 0016); pending completion/always flags are a bitmask.
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
