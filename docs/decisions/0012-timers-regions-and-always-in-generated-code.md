# 0012. Timers, invokes, regions, completion and `#[always]` in generated code

Status: accepted
Date: 2026-11-12
Spec: 5.3, 5.4, 5.6, 5.9, 5.10, 6.5, 7.1, 7.2; ADRs 0005, 0008, 0011

## Context

ADR 0011 fixed the shape of generated code and M3 built it up to hierarchy, history and constructors. Timers and invokes, `cx.raise` and `defer!()`, parallel regions, `on_done` and `#[always]` were still open in detail: the closures of `#[after]` and `#[invoke]` need a parameter type, the effect order of exits, cancellations, entries and scheduling has to be pinned, a region handler needs to tell the parallel state whether its edge leaves it, and the single "always" flag of ADR 0011 cannot say which state is due. Scenarios B and D (hand-written in M2) are the reference; both now run on the macro with the same tests.

## Decision

### Timers and invokes

- A closure of `#[after(duration => event)]` or `#[invoke(task)]` receives the data of the state **by shared reference**: `&T` for a newtype variant (`&Struct` for a compound or parallel state: the whole struct, substates and regions included), `&<Variant>Ref<'_>` for a struct-like or tuple variant (a generated view with `&` fields, `BackoffRef { attempt: &u8 }`), `&()` for a unit variant. An expression that is not a closure is used as is. Shared, because the event is built from `&self` when the timer fires (spec 7.1); the same closure is evaluated on entry for the duration and the task.
- The generated `with_data(data, closure)` helper fixes the data type before the closure is checked, so `|backoff| backoff(*backoff.attempt)` needs no annotation.
- One epoch per state with timers or invokes, a `u32` field of the runtime. The key of timer or invoke `i` is `(StateId, epoch, i, ChildPath::ROOT)`.
- The order of a transition is **exits, cancellations, write, entries, epochs and scheduling** (spec 5.3 steps 1, 2, 7, 8, 9): every user effect of the exits comes first, then one `CancelTimer`/`Cancel` per timer and invoke of the exited states (deepest first, regions in reverse order, timers before invokes), then the user effects of the entries, then for each entered state (parents first, regions in declaration order) `StartTimer`s and `Invoke`s. This is the order of the hand-written scenarios; per children enum there are `exit_*`/`enter_*` (user code) and `cancel_*`/`start_*` (tasks), the latter only when something below has tasks.
- `timer_event` and `accepts_invoke` walk the active configuration from `&self`, compare the key with the epoch of the runtime and build the event from the current data.

### Parallel states

- `*Fields` and `split()` exist for parallel states too: `split` returns the shared fields and one `&mut` per region. A region is a children owner like a compound state: `exit_<RegionEnum>`, `enter_<RegionEnum>`, a walk binding `<region>_substates`.
- `dispatch_<parallel>` runs `evaluate_<region>` in declaration order and folds each `Outcome` into a `RegionTally` (consumed, deferred, transitioned, changed, left, stopped). `resolve_region` is `resolve` plus `tally.left`, set from `RegionEdge::leaves_parallel`, a method the macro implements for the edge enum of every handler inside a region (variants whose LCA is above the region). Handlers of a region with a constructor edge return `impl __estado::RegionEdge` instead of `impl Edge<Self>`.
- After a region-internal transition, `cx.refresh_active(path)` runs only when `TRACKS_ACTIVE` and only for regions that are not the last (ADR 0008).
- A state with `#[unhandled(policy)]` inside a region stops the event: the other regions still look, the handler of the parallel state does not run, the outcome is `Unhandled(policy)` (spec 5.1 item 4).
- `changed` of a region-internal self or ancestor transition compares the leaf of the region (`region_leaf_<region>`), because `state_id()` of a parallel state is the parallel state itself.
- `goto!` path syntax skips regions: `goto!(Active { .. } / On { .. } / Bright)` enters `Bright` in its region, the other regions start in their `#[initial]`.
- Not supported yet, E0090: a parallel state inside a region (the short-circuit would need a level per parallel state), a handler on a region itself, and a local transition whose LCA is a parallel state. A transition between sibling regions is E0021 (spec table 6.7).

### Completion and `#[always]`

- **Amended by ADR 0013 item 4:** the single `completion_pending` flag is replaced by one pending flag per state that can complete (`<state>_done_pending`), so a completion fires exactly once per entry into a final configuration and a sibling region completing does not run it again. The next two bullets describe the original design.
- The runtime keeps `completion_pending` (set when a `#[terminal]` below the root is entered) and one `<state>_always_pending` flag per state with an `#[always]` handler (set by `start_*` when the state enters, cleared by `cancel_*` when it exits, so a set flag always names an active state). `take_always_pending` reports whether any is set; `run_always` clears the flag it serves.
- **Amended by bead 4.10:** the per-state `bool` fields (`<state>_done_pending`, `<state>_always_pending`) are bits of one `pending: PendingFlags<N>` field, completions first, then the `#[always]` states, `N = ceil(flags / 64)`. `take_always_pending` is then one comparison for up to 64 flags instead of an `||` over one byte per state, the runtime stops growing with the number of states, and a machine with more than 64 flags just gets more words (no limit, no diagnostic).
- `run_always` serves one thing per call: the completion check first (innermost completion first: a compound state inside a region before the parallel state; since ADR 0013 each has its own flag, nothing is "checked again"), then the `#[always]` of the deepest state that is due. An `#[always]` runs once per entry, not on every later pending flag.
- `#[on_done(P)]` on a parallel state fires when every region's active child is a `#[terminal]`; a region without a terminal never completes. It receives the `*Fields` of `P` and no `Output` (ADR 0005).

### Inline

- `Spec::microstep` of the impl is `#[inline(always)]`, like the function it calls. Without it the 700-line body is too big for LLVM to inline into `send_batch` and BM-01 goes from 1.7x to 2.6x of the hand-written baseline (spec errata 10).

## Consequences

- Scenarios B and D run on the macro with the same test cases as the hand-written versions (the cases are shared, `examples/tests/*/cases.rs`); the generated code differs from the hand-written code only in naming, in the in-place writes and in `changed` for region transitions.
- `cx.in_state` is always `false` inside an `#[always]` handler: the core builds the `Cx` of `run_always` without the configuration snapshot (spec errata 11).
- A struct-like variant with timers or invokes gets a public `*Ref` view next to its `*Mut` view.
- Test machines with dead ends or unreachable states need `allow(W0001, W0002)`: both warnings are wired now (ADR 0003).
