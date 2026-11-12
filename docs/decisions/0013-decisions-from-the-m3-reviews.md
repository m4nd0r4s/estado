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

## Amendments

Decisions of the fix round (bead estado-99i.4.8) that go beyond the text above, or read it more precisely. "Item N" is the number of a decision above; "finding N" is the number in the consolidated findings list of the reviews.

1. **Item 1, mechanism.** Every machine module gets one hidden `use <path> as __estado_core;`, where `<path>` is `::estado::__private::core` or `::estado_core`, and the generated code spells `__estado_core::Next`, never `::estado_core::Next` (`estado-macros/src/core_crate.rs`). The facade-only pass test is the crate `crates/estado-facade-check`, which depends on `estado` and on nothing else: an integration test inside `crates/estado` would still see `estado-core`, which is one of its dependencies, and pass before the fix.
2. **Item 4, flags.** The pending flag exists for every compound state with a `#[terminal]` substate and every parallel state with a `#[terminal]` directly under one of its regions, whether or not it has an `#[on_done]`: entering the terminal still costs the completion microstep that ADR 0005 pins (`Report.microsteps`). A state without `#[on_done]` serves its flag as a microstep that does nothing. A `Next` that is neither `stay!()` nor `goto!`, which E0045 cannot see when it comes out of a helper function, ends the completion or `#[always]` microstep as `Stayed`.
3. **Item 2, scope of the search.** The nearest `#[unhandled(p)]` is searched from the state through its ancestors up to the top of its region. Inside a region the search stops there, because the parallel state decides after every region looked (spec 5.6).
4. **Item 5, which edges compare the configuration.** Not only `reenter!()` and external self-transitions: every edge whose target is the handler's state or an ancestor does. The probe is the leaf of the region when the transition stays inside a region, the leaf id when the machine has no parallel state (the same answer, cheaper; BM-01 has none), and `path()` otherwise.
5. **Item 9, privacy.** Views copy the visibility of each field of the struct they borrow; the `*Mut` and `*Ref` views of variants keep the visibility of their enum. `reborrow` stays a public method of the view; `split` becomes the private, hidden `__estado_split`. The runtime struct (and its `Runtime` impl) moves out of the hidden module into the machine module, with the visibility of the root type and private fields, because the associated types of the `Runtime` impl cannot name a type that is more private than the impl (E0446, not silenced by an `allow`). `private_bounds` is allowed next to `private_interfaces` on the hidden module.
6. **Item 10, names.** E0012 for the id constant only compares it with the variants of the root enum (that is the only place a constant and a variant share a name). A numbered edge variant skips the numbers that would equal the name another state gets (`ToFoo2` for a state `Foo2`) instead of using a different separator. Generated names use `plain_name`, the identifier without `r#` (a state `r#loop` has the view `loopMut`, the graph name `loop`).
7. **Item 11, E0046 scope.** Only the parameters that carry the data of the state (own data, `*Mut` view, `*Fields` of ancestors) are checked, by name and syntactically (path expressions and the tokens of macro calls in the closure body); names the closure binds itself are not reported. A local used by reference is not E0046: the docs page says to use `move`.
8. **Item 8, forced features.** `tracks_active` is a flag; `raise_cap` and `defer_cap` turn the queues on when written, even to their defaults. The module docs of `#[machine]` say so.
9. **Finding 15, hygiene.** The locals `epoch`, `children`, `runtime` and `key` of the functions that paste user expressions have `Span::mixed_site()`. `cx` and the bindings named after the states (`<state>_state`, `<state>_<field>`) do not: a user function with one of those names is still shadowed, which would need every generated use of `cx` to change.
10. **Finding 16, limits.** More than 255 `#[after]` timers or `#[invoke]` tasks on one state, or more than 256 substates in a compound state with shallow `#[history]`, is E0090.
11. **Finding 18, spans.** A diagnostic carries an `Extent` (first and last token) instead of one span, so a `goto!(..)`, an attribute or a return type is underlined whole. `goto!()` without a target is spanned at the macro.
12. **Finding 21, E0017.** `#[handlers]` on anything but an inherent `impl` of the root type, and a role attribute on a constant, a type alias or a macro call of the impl, are E0017. A block on another type is emitted without its functions, so rustc has nothing to add to the error.
13. **Item 12 (`#[initial(with = ..)]`, finding 14) is not implemented in this round.** What is left, and why:
    - the value of a level is built in the decision phase and carried by the edge; calling a constructor that takes `cx` and the fields of the parent at entry needs the parent value to exist before its child, so the edge would have to carry the pieces the handler supplies and build the value in `apply` (a generated parts struct per `goto!`, plus the same call in `reenter!()`, region reset and history fallback);
    - the start chain cannot call a constructor that takes `cx` without a change in `estado-core`: `Spec::initial()` has no `Cx`, so a root-level `#[initial(with = ..)]` with the signature of the ADR (`fn(&mut Cx<Self>) -> Data`) needs `initial` to receive one (or `enter_initial` to be allowed to replace the root value).
    Until then `with = path` keeps its M3 shape: a function without parameters that returns the value of the enum it is the initial of (`fn() -> ActiveSub`), evaluated where the value is built.
14. **Finding 22, consolidation.** One `FACADE_CRATES` and `facade_name`, one `SINK_PARAMETER`, `exiting_name`, `MachineOptions::effect_type` / `ctx_type`, `StateTree::children_owners` / `any_below` / `has_parallel`, `generate::naming::{children_function, state_constant, hygienic_local}`, and in `walk` the `FieldsAccess` argument, `split_payload`, `state_function` and the field-pair helpers replace the copies the review listed.

