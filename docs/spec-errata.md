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

## 10. 6.5: `microstep` is `#[inline(always)]`

- **Problem.** 6.5 says the generated code does not force `#[inline(always)]`. ADR 0009 allowed an errata entry if BM-01 needed it. The `microstep` of a parser machine is large because every handler and the per-composite dispatch are inlined into it, and without a hint LLVM does not inline it into the `send_batch` loop.
- **Resolution** (ADR 0012). `Spec::microstep` of the generated impl, `__estado::microstep`, `resolve`, the bubbling functions and `dispatch_<parallel>` carry `#[inline(always)]`; user handlers keep `#[inline]`. Measured on scenario A (1 MB of requests, release, local Apple M-series, a busy machine, so the ratios matter more than the milliseconds): the macro-generated `send_batch` took 2.6 ms without the hint on `Spec::microstep` and 1.6-1.8 ms with it, against 1.0 ms for the hand-written baseline.

## 11. 4.3, 5.9: `cx.in_state` inside an `#[always]` handler

- **Problem.** 4.3 says `in_state` answers from a snapshot of the active configuration. `Machine` builds the `Cx` of `run_always` (spec 4.1) without the snapshot, so `cx.in_state(..)` is `false` there even when `Spec::TRACKS_ACTIVE` is true. Handlers and entries of the microsteps for events do have it.
- **Resolution.** Fixed in `estado-core`: `drain` takes the snapshot for `run_always` exactly as `microstep` does when `TRACKS_ACTIVE` (test `in_state_inside_always_sees_sibling_regions`). `#[always]` and `#[on_done]` handlers may call `cx.in_state`.

## 12. 6.1, 7.1, 7.2: the closures of `#[after]` and `#[invoke]` take the data of the state by reference

- **Problem.** The spec shows `|s: &Connecting| s.timeout()` for a state with a named struct and says nothing for struct-like variants, unit variants, or compound states, and the event of a timer must be built from `&self`, where a `&mut` view cannot exist.
- **Resolution** (ADR 0012). `&T` for a newtype variant, `&Struct` for a compound or parallel state, a generated `&<Variant>Ref<'_>` for struct-like and tuple variants, `&()` for unit variants.

## 13. 10.3, BM-01: the 5% target is not met, and the gap is the machine layer

- **Problem.** ADR 0009 started M3 with BM-01 open and asked for a re-measurement against the macro-generated parser (`a_http_parser_macro`). Three local release runs of `cargo bench -p estado-examples --bench bm_01_http_parser` (1 MB of requests, Apple M-series, a busy machine, so read the ratios):

  | Measurement | Time per MB | Ratio to the hand-written baseline |
  | --- | --- | --- |
  | hand-written baseline | 0.98-1.00 ms | 1.00 |
  | M2 hand-written estado, `send_batch` | 1.73-1.77 ms | 1.77-1.79 |
  | macro-generated estado, `send_batch` (`estado_macro_send_batch`) | 1.64-1.83 ms | 1.68-1.83 |
  | M2 hand-written estado, `microstep` called directly | 1.20-1.39 ms | 1.20-1.41 |
  | macro-generated estado, `microstep` called directly | 1.23-1.26 ms | 1.25-1.27 |

- **Reading.** The generated code is as fast as the hand-written code of M2 (in-place writes and the forced inline of ADR 0009 are in, see entry 10), and both are about 1.25x of the baseline when `Machine` is left out. The rest of the distance to the baseline, 1.7x in total, is the per-event cost of `Machine::send_batch` (poison guard, report, queue checks around every microstep), which lives in `estado-core`.
- **Resolution.** Open, for the owner of the spec. Either revise the target of BM-01 for a one-event-per-byte machine to about 2x of a hand-written parser, as ADR 0009 allows, or take the loop of `send_batch` out of `Machine` (one running report, no per-event guard), which touches the BM-02 layout that M2 found sensitive. This entry does neither; the BM-01 test does not assert the ratio.

## 14. 6.7: new error codes E0042-E0046 and E0090 (not in the table of 6.7)

- **Problem.** The macro reports six conditions the table of 6.7 has no code for, or whose code the spec uses for something else (E0041 is the constructor form in `#[edges]`, which this version does not support).
- **Resolution** (ADR 0013 item 13). Each has a page in `docs/errors/` and a `trybuild` snapshot in `crates/estado-macros/tests/ui/`:
  - `E0042`: a transition macro in an `#[entry]` or `#[exit]` function (spec 5.3: they MUST NOT transition).
  - `E0043`: a malformed `goto!`: the target path is not nested, `external` with a target that is not the state or an ancestor, `history` on a state without history, an `Exiting![X]` that names the wrong state, a constructor with other than one parameter, or arguments given to `stay!()` and the other macros without any.
  - `E0044`: `#[on_done(S)]` on a state without substates or regions, which never completes (spec 5.10).
  - `E0045`: `reenter!()`, `defer!()`, `bubble!()` or `unhandled!()` in an `#[always]` or `#[on_done]` handler. Those handlers react to their state, not to an event, so only `stay!()` and `goto!` mean something (ADR 0013 item 3).
  - `E0046`: the constructor closure of `goto!(T, |old: Exiting![X]| ..)` uses the data, view or `*Fields` parameters of its handler. The constructor runs after the handler returned (5.3 step 5); rustc would say E0700 with advice that cannot be followed (ADR 0013 item 11).
  - `E0090`: the feature is part of the planned macro language but not generated yet (`#[choice]`, nested parallel states, a handler on a region, nested deep histories, counts that do not fit the `u8` of an index, ..). It is the code of every "not supported yet", so that an unsupported case never compiles to something half working.
- E0012 also reports a state whose name equals its own id constant (`IO`), and E0017 a `#[handlers]` block on a type other than the root and a role attribute on a non-function (ADR 0013 item 10; review finding 21).

## 15. 6.1: `#[initial(with = path)]` takes no arguments

- **Problem.** 6.1 gives the initializer the signature `fn(&mut Cx, &ParentFields) -> Self`. A constructor that needs `cx` and the parent's fields can only run inside the transition, after the exits, when the parent value does not exist yet; the edge would have to carry a generated "parts" struct per `goto!`. A root-level initial has no `Cx` at all, since `Spec::initial()` runs before the machine starts.
- **Resolution** (lead decision, ADR 0013 item 12 and its amendments). In v0, `with = path` names a function with no arguments that returns the state's value; it is called when the state is entered through its initial. The signature of the spec is left for a later version, as an additive change (a second attribute form), so machines written today keep compiling.

## 16. 6.5: the `StateId` constants end in `_ID`

- **Problem.** 6.5 names the id constants after the state (`Player::PLAYING`). The constants are associated items of the root type, which is also the root enum, so a root-level state named `A` or `IO` is a variant and a constant with the same name. The macro rejected such names with E0012, which makes the common single-letter and acronym names unusable.
- **Resolution** (bead 4.14). The constant is the name in SCREAMING_SNAKE_CASE with the suffix `_ID`: `Player::PLAYING_ID`, `Bus::IO_ID`. The suffix cannot clash with the name of any state (they are CamelCase or acronyms without the suffix); the only clash left is a variant that is itself called `<STATE>_ID`, which stays E0012. All docs and examples use the new names.
