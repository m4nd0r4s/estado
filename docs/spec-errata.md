# Spec errata

Contradictions and errors in the spec. By rule 2 of section 0, section 5 wins. Each entry: affected section, problem, resolution.

## 1. AC-15 and 3.5: the size requirement ignores alignment and niches

- **Problem.** `size_of::<Machine<M>>() == size_of::<M>() + size_of::<M::Ctx>()` cannot hold even for a plain `struct { M, Ctx }`: alignment padding breaks the sum (`Player` + `u8` is 49 by the formula, 56 in memory). And when `M` has no spare niche value, any poison or started marker costs extra bytes.
- **Resolution** (ADR 0002). AC-15 becomes: `size_of::<Machine<M>>() == size_of::<(M, M::Ctx)>()` for a machine without optional features whose root enum has at least three niche values (`Unstarted`, `Poisoned`, `Finished`; checked as `size_of::<Slot<M>>() == size_of::<M>()`); otherwise the excess is at most `align_of::<M>()`.

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

## 7. 3.2 and 6.1: `#[machine]` is applied to an inline module

- **Problem.** The spec applies `#[machine]` to the root enum and `#[state]`/`#[handlers]` to separate items, but an attribute proc-macro sees only its own item, and generating typed edges, linear exit/entry code and the graph needs the whole machine.
- **Resolution** (ADR 0010). `#[machine(..)]` goes on an inline `mod` containing every item of the machine; the root enum is marked `#[root]`. All other attributes keep their names and meaning. E0001 now means "`#[machine]` not on an inline module with exactly one `#[root]` enum".

## 8. 3.1, 3.3 and 6.7: new error codes for structural rules the spec states without a code

- **Problem.** The spec states structural rules in 3.1/3.3 (a compound has exactly one `#[substates]`, a parallel state has two or more `#[region]`s, `#[substates]` and `#[region]` never mix, the field type is a state enum) and the options and attributes of 6.1 have a fixed syntax, but the table in 6.7 gives none of these a code.
- **Resolution** (bead 4.1). Added to the table, each with a page in `docs/errors/` and a `trybuild` snapshot:
  - `E0013`: a `#[state]` struct combines `#[substates]` and `#[region]` fields, or has more than one `#[substates]` field.
  - `E0014`: a `#[state]` struct has exactly one `#[region]` field (a parallel state needs at least two).
  - `E0015`: the type of a `#[substates]` or `#[region]` field is not a `#[state]` enum of the same module.
  - `E0016`: invalid `#[machine(..)]` option: unknown name (with a "did you mean" help), repeated, bad value, or a flag given a value.
  - `E0017`: malformed state attribute (`#[initial(..)]`, `#[after(..)]`, `#[invoke(..)]`, `#[history(..)]`, `#[unhandled(..)]`), or a marker on the wrong item (`#[history]` on a variant, `#[initial]` on a struct, `#[state]` on a function).
- E0010 is also reported when a level has more than one `#[initial]` variant ("exactly one", 3.2). Until the edges are resolved (4.2), a level without `#[initial]` is always E0010, although 3.2 allows omitting it when no `goto!` needs it.
- A region counts as a level for the depth limit of E0011 (it is a node of the graph), and a region name clashes (E0012) with a state whose `StateId` constant is the same (`link` and `Link` both give `LINK`).

## 9. 3.2 and 6.1: `#[final]` is not valid Rust syntax

- **Problem.** `final` is a reserved keyword. rustc rejects `#[final]` while parsing the module ("expected identifier, found reserved keyword `final`"), before any macro runs, so the attribute cannot be written as the spec shows.
- **Resolution** (user decision, bead 4.1). The attribute is named `#[terminal]`. Documentation keeps the term "final state"; `StateKind::Final`, `is_done`, `on_done` and the graph are unchanged.
