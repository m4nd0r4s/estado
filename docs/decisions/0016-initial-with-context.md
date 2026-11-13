# 0016. `#[initial(with = f)]` takes the `Cx` and the parent fields

Status: accepted
Date: 2026-11-13
Spec: 6.1; errata 15; ADR 0013 items 12 and 13; bead `estado-99i.4.9`

## Context

Spec 6.1 gives the initializer of a variant the signature `fn(&mut Cx, &ParentFields) -> Self`, called when the state is entered. Version 0 kept a function without parameters (ADR 0013 item 13) because the macro built the value of the child of the LCA in the decision phase, inside the handler: there is no `cx` there, the transition has not exited yet, and the parent value does not exist before its substates. A root-level initial had no `Cx` either, since `Spec::initial()` runs before the machine does.

A function that emits effects or reads the context must run where the entries run, after the exits of the same transition, so that its effects are ordered with theirs and it sees the context the exit handlers left.

## Decision

**Two forms, picked by signature.** The zero-parameter form works as before. The macro reads the signature of the function named by `with` from the machine module (a free function, or a function of the root's `#[handlers]` impl through `Self::f` or `Root::f`) and recognises the parameters by type: `&mut Cx<..>` and `<Parent>Fields<'_>`, in any order, each optional. A path it cannot see is called without parameters, so every machine written before keeps compiling. Another parameter is `E0017`.

**`ParentFields`** is the `<Parent>Fields<'_>` view of ADR 0007, by value: `&mut` references to the shared fields of the state that owns the level, that is the compound state whose `#[substates]` it is, or the parallel state of a `#[region]`. It is not the whole parent struct, which does not exist yet while its substates are built. The root enum has no parent and takes at most the `Cx`.

**The return type** is the enum value, as in the zero-parameter form, not only the data of the variant: the function builds the substates of the initial too, so one function serves a compound initial. This reads the `Self` of 6.1 as "the level" and drops the `Data` of ADR 0013 item 12.

**Root level.** `Spec` gets a provided method `initial_in(cx) -> Self`, defaulting to `initial()`. `Machine::start` calls `initial_in` inside the start, with a `Cx` whose `in_state` reports no active state, before `enter_initial`. It is inside the poison guard of the start, so a panic poisons and a fault of the sink or the raise queue is returned and poisons. `#[machine]` implements `initial_in` when the root initial takes the `Cx`, and then `initial()` panics with a message that names `Machine::start`.

**Nested levels** are built in the place that has a `cx`:

- `Spec::initial_in` for the initial chain of the root;
- `reenter!()`, which restarts the substates after the exits, borrowing the parent fields from the struct being reset;
- `Edge::apply` for a `goto!`. The handler cannot build the value, so it evaluates the data the user wrote (in the user's scope) and the edge variant carries those values (`GotoForm::Deferred`); `apply` builds the value after the exits and before writing it, binding the shared fields of each level as locals so that the initializer can borrow them. A local `goto!` to an ancestor has no data of its own: the initializer gets the live fields that the walk of `apply` already bound.

A transition whose value needs no such function is built exactly as before.

**Not supported yet:** `goto!(S, history)` into a state whose initial takes parameters is `E0090`. The edge builds the initial value before the history override replaces it, so the function would run, and emit its effects, even when a history is recorded.

## Consequences

- Effects of an initializer come after the effects of the exits and before those of the entries of the states it builds.
- An initializer is user code of the step: a panic in it poisons the machine, and an `EffectOverflow` or `RaiseOverflow` it causes is reported like any other.
- The function is a plain function, not a handler, so the macro does not complete `Cx<Self>`: write `fn f<S: Sink<Effect>>(cx: &mut Cx<'_, Root, S>, ..)`.
- Tests: `crates/estado-macros/tests/initial_with_context.rs` (goto with data, `reenter!()`, local goto to an ancestor), `initial_with_context_root.rs` (root, chain, panic), `initial_with_context_regions.rs` (regions), `crates/estado-core/tests/initial_in.rs` (core), and the `e0017_initial_unknown_parameter` and `e0090_history_into_context_initial` UI cases.
