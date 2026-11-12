# 0008. Evaluation of parallel regions (open item 13.4)

Status: accepted
Date: 2026-11-12
Spec: 4.3, 5.4, 5.5, 5.6, 13.4, risk R6; errata 6; ADR 0005

## Context

Spec 5.6 evaluates the regions of a parallel state `P` **sequentially, with short-circuit**:

1. regions run in declaration order;
2. a transition internal to a region is applied at once, before the next region runs (errata 6 and ADR 0005 add: and `cx.in_state` is refreshed);
3. a transition out of `P` is applied and the remaining regions are not evaluated;
4. the event is consumed if at least one region returned `stay!`, `goto!` or `reenter!`;
5. if none consumed it and one returned `defer!`, it is deferred; otherwise it bubbles to the handler of `P`.

Open item 13.4 asks whether to offer, or switch to, the alternative where **all regions see the event before any is applied** (`#[regions(all_see_event)]`): every region decides against the configuration of the start of the microstep, then the decisions are applied in declaration order. That is closer to SCXML. The criterion in the spec is: prototype with scenarios B and D, and keep the current semantics if no real case breaks it.

Both prototypes were run, without touching `estado-core`: the dispatch of each scenario was temporarily replaced by a two-phase version (collect the `Next` of every region, then apply them in order, short-circuit on a transition out of `P`; no `refresh_active`, since nothing is applied between regions), the test suites were run, and the original dispatch restored.

## Evidence

### Scenario B (connection: `link`, `keepalive`)

Reading the handlers (comment at the top of `examples/b_connection/dispatch.rs`) and running the prototype agree: the two semantics give the same states, effects, timers and defer behavior. With the two-phase dispatch **38 of 39** tests of `examples/tests/b_connection` pass. The one failure asserts the interleaving of the trace (`handler Sending, exit Sending, entry Idle, handler Pinging, ..` versus all handlers first, then the exits and entries); the resulting states and effects are identical.

Why B is insensitive:

- `FrameSent` is wanted by both regions and `keepalive` does not read `link`;
- `link` reads `keepalive` (`cx.in_state(AwaitingPong)`) but is evaluated first, so it reads the start-of-microstep configuration under either semantics;
- the only transition out of `Connected` (`Dead`) is in the last region, so the short-circuit skips nothing.

B also shows the hazard of items 4 and 5: a region that consumes an event discards a `defer!()` another region returned for it. B avoids it by choosing `FrameSent`, not `Send`, as the activity signal for `keepalive`. The hazard is the same in both semantics; all-see-event does not remove it.

### Scenario D (character: `movement`, `weapon`, `anim`)

`anim` has no state of its own to decide: its pose is a function of what `movement` and `weapon` did **with the same event**. Each pose handler answers the events that can change the pose by asking `cx.in_state(FIRING | JUMPING | WALKING)`. This needs the sequential semantics:

- `Jump` is turned by `movement` into `Jumping`, applied, and `anim` (third) reads `Jumping` through the refreshed snapshot;
- `Fire` makes `weapon` go `Ready -> Firing` only when the magazine and the trigger allow it; `anim` reads the *outcome* instead of repeating the guards;
- `Land` (a timer) and `CooldownElapsed` work the same way.

With the two-phase prototype **15 of the 46** tests of `examples/tests/d_game_character` fail, among them the invariant "the pose is always the one derived from the other regions" of the randomized run, on its first `Move`: `anim` decides against the old configuration, so it is one event behind: after `Move` it is still `Idle`. The only way back under all-see-event is for every pose handler to match on the event and repeat the guards of the other regions (`Fire` only if `weapon` is `Ready` with rounds left, which `anim` cannot even see ahead of time without reading `ammo`). That is the duplication regions exist to avoid.

Two other places where D touches 5.6:

- **Preemption does not depend on the choice.** A fatal `Damage` is handled by `Alive` itself (item 5): no region consumes it, so it bubbles, and `goto!(Dead)` has the root as LCA. All three regions exit in reverse declaration order and every timer is cancelled (5.3). No region leaves `Alive`, so item 3 is never exercised in D.
- **Item 4 is a design rule, not a bug.** `Damage`/`Heal` reach `Alive` only because no region consumes them: one `stay!()` for `Damage` in a region and the damage is lost. For the same reason only `weapon` handles `ReloadRequest` (a `defer!()` of `weapon` while airborne would be dropped by a `stay!()` anywhere else), and `anim` returns `bubble!()` rather than `stay!()` when the pose is already right: it did nothing, so it must not claim the event.

## Decision

**Keep sequential evaluation with short-circuit, exactly as in 5.6. Do not add `#[regions(all_see_event)]` to v0.** 13.4 is closed.

**Items 4 and 5 are unchanged.** No errata entry: the behavior is what section 5 says, and the loss is visible and avoidable in the machine (see the rules above). A deferral surviving a consumption in another region would need a rule for what the consumed event then means (consumed and deferred at once), which no scenario asks for.

Re-open only if a real machine needs a region that must not see the transitions of the regions before it, or regions that are order independent by contract. The option can then be added as an opt-in attribute without changing the default.

## Consequences

- **Declaration order is semantics.** A region that derives its state from siblings (`anim`) must be declared after them. The documentation of `#[region]` says so, with the D pose example.
- A region reads the **applied** state of regions declared before it and the **start-of-microstep** state of regions declared after it. Entries and exits of a transition see the start-of-microstep snapshot (ADR 0005, point 4).
- A region cannot both transition and let the parent handler of `P` run: if it consumes the event, `P`'s handler never sees it. To make the parent react after a region reacted, the region raises an internal event (`cx.raise`) that the parent handles. This and the defer-drop of items 4 and 5 go in the documentation section that risk R6 asks for (comparison with SCXML and XState, where all regions see the event), with the B and D examples.
- Follow-up for M3, a candidate for a new warning in 6.7: the macro can see, from literal event patterns, an event that one region of a parallel state handles with `stay!`/`goto!`/`reenter!` and another defers. This is the one silent loss in the semantics. It is not a new bead yet; it is recorded here for the M3 diagnostics pass.
- The cost of ADR 0005 on a hot path with `TRACKS_ACTIVE` is one `StatePath` rebuild per region-internal transition. The macro knows statically which region is last and skips the refresh for it (D's `anim`; B's `keepalive` does not, in the hand-written code kept as the simplest form). It is an optimization with no behavior change.
- Choices D made on the way, not open items of the spec: `ReloadRequest` while airborne is **deferred**, not ignored, so landing starts the reload the player asked for (a full magazine does not defer: there is nothing to reload); the defer queue of D uses `defer_overflow = drop_oldest` because every deferred event is the same request. State names must be unique across all levels (3.2, E0012): the spec text for D, with an `Idle` in both `movement` and `anim`, would be rejected, so D names the first one `Standing`.
