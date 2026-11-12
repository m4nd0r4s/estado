# 0007. Views of struct-like variants (open item 13.2)

Status: accepted
Date: 2026-11-12
Spec: 3.2, 6.2, 6.3, 13.1 (item 13.2), risk R3

## Context

Spec 6.2 says a state with data SHOULD be a newtype variant (`Playing(Playing)`, handler parameter `&mut Playing`) and that for struct-like variants (`Playing { pos }`) the macro generates a view `PlayingMut<'a> { pub pos: &'a mut Duration }`. Open item 13.2 asks which of the two to keep: the generated view, or requiring newtype variants. The criteria are ergonomics in scenario I and readable compile errors.

Scenario I (`examples/i_media_player/`) was written by hand with struct-like variants on purpose: `ActiveSub::Playing { pos }`, `ActiveSub::Paused { pos }` and `Player::Failed { reason, buffer }`. It uses the generated code in every place a view appears: handlers (`on_playing(PlayingMut, ActiveFields, cx, event)`), entries and exits, the dispatch, and the constructor of the edge `Active -> Failed`.

## Decision

The macro generates the view; newtype variants stay supported and are the recommended shape only when the payload is a type the user wants anyway.

1. For a struct-like variant `V { a: A, b: B }` of a state enum, the macro generates `VMut<'a> { pub a: &'a mut A, pub b: &'a mut B }`, and the handler, entry and exit of `V` take `VMut<'_>`. A tuple variant `V(A, B)` gets a tuple struct `VMut<'a>(pub &'a mut A, pub &'a mut B)`. A unit variant gets no view and no parameter.
2. The dispatch builds the view by destructuring the enum, `ActiveSub::Playing { pos } => on_playing(PlayingMut { pos }, ..)`; with the `*Fields` split it is one `let (up, sub) = active.split()` and one `match`. No unsafe, no extra borrow rules.
3. A constructor that exits a struct-like variant (`goto!(Active, |old: Exiting![Failed]| ..)`) receives a generated owned struct `VExiting { a: A, b: B }`, because the variant has no type of its own. For a newtype variant `Exiting![V]` is the payload type, as in the spec.
4. Parameters are recognized by type, so the macro matches the name `<Variant>Mut` (and `<Struct>Fields`). If a handler asks for `&mut V` and `V` is a struct-like variant, the macro emits its own error naming the view to use; this is a better message than the compiler's "cannot find type `V`". A user type that already has the name `VMut` is an error at the variant, not a silent shadowing.
5. Views and `*Exiting` structs are re-exported next to `*Fields` (spec 6.5). The views are `pub` but plain: no methods, no traits.

## Consequences

- No boilerplate for the user: the README shape (`Playing { pos: Duration }`) works as written, and every state keeps a single, flat definition.
- One more generated type per data-carrying variant, and one more per such variant that is the source of a constructor edge. Both are zero-cost: the view is a struct of references built at the call site and optimized away.
- A view cannot carry methods of the user. Logic shared by handlers goes into free functions or into an `impl` on the view type written by the user. Users who want an inherent `&mut self` API per state should use a newtype variant; that is the recommendation in 6.2 and it stays.
- Views make the split borrow trivial for leaves, but they do not solve the limit of risk R3 (below).

## Risk R3 validation (`*Fields` split borrow)

What worked: the dispatch that passes `PlayingMut` and `ActiveFields` at the same time is mechanical (destructure once, no reborrow) and handlers read naturally; `Event::Tick` mutates `pos` and `buf` in the same handler, and `Event::Seek` reads `*up.track` for the command. The constructor takes `buf` out of the exiting `Active` by partial move.

What was awkward, in order of how much it hurts:

1. **A child handler can never get the whole `&mut Active`.** Its own data lives inside `Active::sub`, so `fn on_playing(p: PlayingMut, whole: &mut Active)` cannot be called (E0499, shown by the `compile_fail` doctest on `ActiveFields`). Consequence: every helper meant to be shared by the handlers of `Active` and its substates must be written on `ActiveFields` (e.g. `ActiveFields::seek_buffer`), not as `Active::method(&mut self)`. Helpers that need both the fields and the substate value must take the pieces as separate arguments.
2. **`ActiveFields` is not `Copy` and not `Clone`.** Passing it by value to a helper moves it; the handler then cannot use `up` again. The macro should generate `reborrow(&mut self) -> ActiveFields<'_>` (the example has it); helpers that take `&mut ActiveFields<'_>` avoid the issue.
3. **The `Fields` view hides the substate on purpose, so a child cannot ask what its siblings are.** It must use `cx.in_state` or keep the information in its own data.
4. **The handler of the compound state itself.** The mitigation in R3 ("`#[on(Active)]` with full access") is technically possible, because no child borrow exists while `on_active` runs, but it lets the user overwrite `active.sub` and skip the exits and entries of the old substate, which breaks 5.3. Decision for the macro: the handler of a compound state gets `ActiveFields<'_>` like its children, never `&mut Active`. When the user really wants a different substate they write `goto!(Active { .. } / Paused { .. })`. A read-only `&Active` may be offered later without this problem; it is not needed by scenario I.
5. **Partial moves out of `Exiting![Active]` need `Active` to have no `Drop` impl** (E0509 otherwise). Worth a line in the macro docs, not a reason to change the design.

Conclusion: R3 is real but small. It costs one rule ("shared helpers go on `Fields`") and the generated `split` and `reborrow`. No change to the spec's mitigation is needed beyond item 4: do not offer `&mut Active` to handlers.
