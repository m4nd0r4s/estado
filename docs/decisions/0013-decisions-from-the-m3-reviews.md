# 0013. Decisions taken from the M3 code reviews

Status: accepted
Date: 2026-11-12
Spec: 4.5, 5.1, 5.4, 5.9, 5.10, 6.1, 6.3, 6.5, 6.7; ADRs 0005, 0010, 0011, 0012

## Context

Two detailed reviews of the macro (bead estado-99i.4.8: commits ba769e2..9eff76b and 18dc7a9..8ad0cdb) found confirmed defects and open questions. The user was unavailable, so the lead decided; this ADR records each decision so the fixes follow one rule set. It amends ADR 0012 (single completion flag) and refines ADR 0005 item 2.

## Decisions

1. **Generated paths go through the facade when it is present.** The macro resolves the crate name with `proc-macro-crate`: if the user crate depends on `estado`, generated code uses `::estado::__private::core::..` (a `#[doc(hidden)] pub mod __private { pub use estado_core as core; }` in the facade); otherwise `::estado_core::..`. A facade-only trybuild pass test guards it.
2. **`unhandled!()` applies the nearest `#[unhandled(p)]` override**, searching the state itself and then its ancestors, the same as `bubble!()` reaching them. Without any override, the machine policy applies. Spec 5.1 item 4 reads "from S"; extending it to descendants avoids the surprise where `_ => unhandled!()` in a leaf silently skips an ancestor's override.
3. **Only `stay!()` and `goto!` are allowed in `#[always]` and `#[on_done]`.** `reenter!()`, `defer!()`, `bubble!()` and `unhandled!()` there are a compile error (new code E0045). These handlers react to the state itself, not to an event, so those results have no meaning.
4. **Completion (`on_done`) uses one pending flag per completing state**, set by the entry of the terminal state that completes it and cleared when served or when the state exits. A completion fires exactly once per entry into a final configuration (ADR 0005 item 2; corrects ADR 0012's single flag).
5. **`changed` compares the full active configuration** (`path()` before and after) for `reenter!()` and external self-transitions, so a re-entry that resets a region requeues deferred events (spec 5.4).
6. **Implicit start drains** (core): `send` on an unstarted machine runs `start` including its `#[always]`, completions and raised events before the external event, exactly like an explicit `start()` followed by `send` (spec 4.5).
7. **Initial entries get the `in_state` snapshot** (core), as event microsteps and `run_always` do (errata 11).
8. **Feature detection can be forced.** `#[machine(tracks_active)]` turns `TRACKS_ACTIVE` on regardless of the syntactic scan, and an explicit `raise_cap` or `defer_cap` always generates the queues. Helpers outside the machine module that call `cx.raise` or `cx.in_state` need these options; the docs say so.
9. **Generated items keep the user's privacy.** Views copy each field's visibility; helper methods generated on user types are `#[doc(hidden)]` and named with an `__estado_` prefix; the hidden module allows `private_interfaces`, since associated types of the `Spec` impl must name the generated runtime.
10. **Names.** A state whose id constant (SCREAMING_SNAKE_CASE) equals its own or another variant name is E0012. Generated identifiers use unraw names. Edge variants are named `To<Target>` plus an occurrence suffix that cannot collide with a state name.
11. **Constructors that capture handler state are reported by estado**, not left to rustc's E0700: the macro scans constructor closures for uses of the handler's state, view and fields parameters and reports E0046 with the fix ("copy the value into a local before the `goto!` and use a `move` closure").
12. **`#[initial(with = path)]` follows spec 6.1**: `fn(&mut Cx<Self>, <Parent>Fields<'_>) -> Data`, called when the state is entered through its initial, not during the decision phase. A root-level initial with data has no parent fields: `fn(&mut Cx<Self>) -> Data`.
13. **Diagnostic codes.** Every code the macro emits is registered in the errata (E0042-E0046, E0090) and has a page in `docs/errors/` and a trybuild snapshot (AC-17).
14. **`#[inline(always)]`** on `microstep` and the per-composite dispatch functions is kept (errata 10 extended to the dispatch functions): BM-01 needs it.
15. **Example machines may exceed the ~400-line file guideline** when ADR 0010 forces the machine into one inline module; helpers that are not part of the machine move out of the module.

## Consequences

- One fix round implements 1-5 and 8-15 in `estado-macros` and the facade, and 6-7 in `estado-core`, each with a regression test that fails before the fix.
- `proc-macro-crate` becomes a dependency of `estado-macros` only; `estado-core` keeps zero dependencies.
