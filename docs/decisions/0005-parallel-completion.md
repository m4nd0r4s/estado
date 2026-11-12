# 0005. Completion of parallel states and `on_done`

Status: accepted
Date: 2026-11-12
Spec: 4.3, 5.4, 5.6, 5.10, 6.1, AC-02, AC-10

## Context

Writing scenario B by hand (`crates/estado-core/tests/support/connection.rs`, tests in `tests/regions.rs` and `tests/regions_and_final.rs`) left four points open:

1. 5.6 item 6 says a parallel state `P` is complete when all its regions are final, but not who reacts. 5.10 defines `#[on_done(C)] fn(&mut CData, &Output, &mut Cx) -> Next` for compound states only.
2. 5.10 calls the reaction a "synthetic microstep" but does not place it in the queue order of 5.4.
3. 5.10 says a root `#[final]` makes `is_done()` true, and `Machine::take_output` returns the payload by value from `&mut M`.
4. 4.3 and 5.6 disagree on what `cx.in_state` shows between two regions (errata 6).

## Decision

1. **`#[on_done(P)]` also works on a parallel state.** It is a handler of `P` itself, exactly like `on_done(C)` of a compound state: its transition has `H = P`, so a `goto!` to a sibling or outside state exits all regions in reverse declaration order and then `P` (5.3). It receives the `*Fields` of `P` and `cx`, with **no `Output` parameter**: each region may end in a different final, so there is no single output. Without `#[on_done(P)]` nothing happens: `P` stays with all its regions in their final states, and events keep going to its handler. Partial completion (some regions final) never fires anything.
2. **The completion microstep travels the `#[always]` channel.** Entering a `#[final]` state that has a parent (a compound substate or a region leaf) sets a flag in the runtime (one flag per completing state since ADR 0013 item 4); the flag is reported through `Runtime::take_always_pending` and `Spec::run_always` checks the configuration: a compound whose active substate is final, or a parallel state whose regions are all final, runs its `on_done`. So the completion runs after the entries of the transition that caused it, before internal events and requeued events (5.4: always > internal > requeue), counts as a microstep and as a transition if it transitions (`max_microsteps`, `Report`), and fires once per entry into a final: a done state whose `on_done` returns `stay!()` does not loop. When one entry completes several levels, the innermost completion runs first, one per microstep. An entry into a root `#[final]` sets no flag: nothing is left to complete and `is_done()` is enough.
3. **Root output is moved out by value.** Superseded during M2: `Spec::into_output(self) -> Result<Output, Self>` moves the payload out without `Output: Default`, and `Machine::take_output` leaves the slot `Finished` (still done, later calls return `None`, later sends are ignored). The `Slot` therefore needs three spare niche values (errata 1).
4. **`in_state` is refreshed after region-internal transitions.** `Cx::refresh_active(path)` (`#[doc(hidden)]`) replaces the snapshot the generated code took at the start of the microstep. The generated code calls it right after each region-internal transition of a parallel state, only when `TRACKS_ACTIVE`. Entries and exits of that transition still see the snapshot of the start of the microstep; handlers always see the current configuration.

## Consequences

- The macro generates one flag write per `#[final]` entry (statically known: only for finals with a parent), and a `run_always` that matches the configuration; no tables.
- `Report.microsteps` includes a completion check that found nothing to do (a compound without `on_done` costs one extra microstep when its final substate is entered). Tests pin it (`ac_10_a_compound_without_on_done_stays_in_its_final_substate`).
- `Cx` carries an `Option<StatePath>` more (stack only, not in `Machine`).
- A parallel `P` that is itself a substate of a compound `C` completes independently of `C`: `C`'s `on_done` only reacts to `C`'s own active substate entering a final, and `P` is not final.
