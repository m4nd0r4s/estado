# Spec errata

Contradictions and errors in the spec. By rule 2 of section 0, section 5 wins. Each entry: affected section, problem, resolution.

## 1. AC-15 and 3.5: the size requirement ignores alignment and niches

- **Problem.** `size_of::<Machine<M>>() == size_of::<M>() + size_of::<M::Ctx>()` cannot hold even for a plain `struct { M, Ctx }`: alignment padding breaks the sum (`Player` + `u8` is 49 by the formula, 56 in memory). And when `M` has no spare niche value, any poison or started marker costs extra bytes.
- **Resolution** (ADR 0002). AC-15 becomes: `size_of::<Machine<M>>() == size_of::<(M, M::Ctx)>()` for a machine without optional features whose root enum has at least two niche values (checked as `size_of::<Slot<M>>() == size_of::<M>()`); otherwise the excess is at most `align_of::<M>()`.

## 2. 3.5 and 4.5: no room for the "started" bit

- **Problem.** 4.5 says `new` runs no entries and `send` starts implicitly, so the machine must know whether it started; the logical content in 3.5 (`state`, `ctx`, `rt`) has no place for it.
- **Resolution** (ADR 0002). The state field is `slot: Slot<M>` with `Unstarted | Live(M) | Poisoned`. `state()` panics on an unstarted machine with a distinct message; `try_state()` returns `None`.

## 3. 3.6 and 5.11: `__Poisoned` variant replaced by `Slot::Poisoned`

- **Problem.** 3.6 asks the macro to add a hidden `__Poisoned` variant and hide it; both hiding options in 13.1 are worse than a core-owned wrapper (no `state()` with a safe mirror, `unsafe` and a bigger enum with a cast).
- **Resolution** (ADR 0002). No hidden variant is generated. The poison guard of 5.11 writes `Slot::Poisoned`. During unwinding the half-changed value is dropped, so a user `Drop` that panics there aborts the process.

## 4. 4.2: `Edge` needs a method besides `TARGETS`

- **Problem.** The generated dispatch cannot name a handler's hidden edge type, so it cannot run the transition code for it.
- **Resolution** (ADR 0001). `Edge` gets a `#[doc(hidden)]` `apply` method holding the transition code of 5.3. Handler return types are `Next<Self, impl Edge<Self> + use<'event, S>>`, and the macro generates one edge variant per `goto!` occurrence.

## 5. 3.4 and 6.3: deep history and constructors both want the substate value

- **Problem.** `#[history(deep, keep)]` stores the full substate value when the compound exits (3.4, 5.3 step 3), and a constructor receives the exit segment by value, substates included (`Exiting![Active]` is `Active` with `sub`, 6.3). Both cannot own the same value without `Clone`.
- **Resolution** (ADR 0006). For a compound with `#[history(deep, keep)]`, the substate value moves into the history and `Exiting![X]` is a generated struct with the remaining fields only. Compounds without deep history keep the spec's shape.

## 6. 4.3 and 5.6: `cx.in_state` snapshot taken at the start of the microstep is stale between regions

- **Problem.** 4.3 suggests a snapshot of the active discriminants "updated at the start of each microstep". 5.6 item 2 applies a region-internal transition immediately, before the next region is evaluated, in the same microstep. A later region that asks `cx.in_state(..)` about an earlier region would see the configuration from before the transition, although the transition already ran (its exits and entries are in the log).
- **Resolution** (ADR 0005). Section 5 wins: after every region-internal transition the generated code refreshes the snapshot with `Cx::refresh_active(M::path(..))` (`#[doc(hidden)]`, only needed when `TRACKS_ACTIVE`), so `in_state` always reflects the applied transitions of the current microstep. Test: `ac_02_a_region_internal_transition_is_applied_before_the_next_region_is_evaluated`.
