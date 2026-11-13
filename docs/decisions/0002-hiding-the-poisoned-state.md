# 0002. Hiding the poisoned state

Status: accepted
Date: 2026-11-12
Spec: 3.5, 3.6, 5.11, 13.1

## Context

A panic between the take and the write of a transition (5.3 steps 4-6), or in any in-place phase (handler, exit, entry), must leave the machine poisoned. The user's root enum `M` must not show a `__Poisoned` variant in `match`, graph or snapshot. AC-15 requires `size_of::<Machine<M>>() == size_of::<M>() + size_of::<M::Ctx>()` with no optional features.

Spike: `spikes/poison/` (removed from the tree after M1; see commit `7d72d6c`) (hand-written machines, 45 poison/leak tests with `catch_unwind` and drop-counting payloads, size tests, all green under `cargo +nightly miri test`). Compared:

| Option | Safe? | `state() -> &M` | Notes |
| --- | --- | --- | --- |
| (a) mirror enum `Repr` with `Poisoned`, safe | yes | **impossible**: no `M` exists in memory to borrow | every send converts `Repr -> M -> Repr`; macro must generate the mirror and both conversions for every root enum |
| (a') mirror + `&Repr -> &M` cast | no | via pointer cast | needs `#[repr(u8)]` on both enums; that pessimises the user's enum (spike: 24 -> 32 bytes for `{Idle, Counting(u32), Failed{String}}`); generated `unsafe` lands in the user's crate and breaks `#![forbid(unsafe_code)]` |
| (b) `Option<M>` + `take()` | yes | `as_ref().expect(..)` | `None` is poisoned; the take already poisons, so only in-place phases need a guard |
| (b') `Slot<M> = Unstarted \| Live(M) \| Poisoned` | yes | `match` | (b) with a named poison variant plus the "not started" state that 4.5 needs anyway; lives in `estado-core`, the macro generates nothing |
| (c) `M` + `bool`, `mem::replace` with a placeholder | yes | `&self.state` after flag check | needs a cheap placeholder `M` (`Spec::initial()` may need `Cx` or allocate); +1 byte |
| (d) `MaybeUninit<M>` + `bool`, drop guard | internal `unsafe` | `assume_init_ref` after flag check | no placeholder; +1 byte; needs a `Drop` impl and a guard with a `holds_value` bit |

(a) and (b) have identical layouts in every measured case: both are "M plus one extra discriminant value", and rustc packs that into a niche of `M` either way. The mirror adds generated code and loses `state()`, with nothing gained.

## Decision

Use **(b')**: `estado-core` owns `enum Slot<M> { Unstarted, Live(M), Poisoned }` as the machine's only state field. `M` is exactly what the user wrote, so `match`, graph and snapshots never see the poison.

- Poison guard: a struct holding `&mut Slot<M>` whose `Drop` writes `Slot::Poisoned`. Created at the start of `send`, disarmed with `mem::forget` at the end (free on the happy path; removed entirely with `panic = "abort"`).
- Steps 4-6: `mem::replace(slot, Poisoned)` moves the old value out; the constructor result is written back to `Live`.
- Transition with the LCA below the root: take the **root** by value, destructure down to the child of the LCA, run the constructor, rebuild the path. Superstate fields (`track`, `buf`) are moved, not cloned; on panic they are dropped by unwinding (no leak, no double drop, verified).
- `state()` panics on `Poisoned` and `Unstarted`; `try_state()` returns `None` for both; `is_poisoned()` matches `Poisoned`.
- No `unsafe` anywhere in the public or internal API.

## Consequences

Measured `size_of` in bytes, release or debug (identical). `(M,C)` is a plain `struct { M, C }`, the real lower bound. Option = `OptionMachine`, Slot = `SlotMachine`, Mirror = safe mirror machine, Flag = flag/uninit machines (same size).

Ctx = `()`:

| M | size_of M | (M,C) | Option | Slot | Mirror | Flag/uninit |
| --- | --- | --- | --- | --- | --- | --- |
| Unit (3 unit variants) | 1 | 1 | 1 | 1 | 1 | 2 |
| Strs (`&'static str`, `NonZeroU32`) | 24 | 24 | 24 | 24 | - | 32 |
| Owned (`String`) | 24 | 24 | 24 | 24 | 24 | 32 |
| Pair `{A(u8), B(u8)}` | 2 | 2 | 2 | 2 | - | 3 |
| Player (nested, spec 3.2) | 48 | 48 | 48 | 48 | 48 | 56 |
| Boxed `{Idle, Data(Box)}` (no niche left) | 8 | 8 | 16 | 16 | 16 | 16 |
| Only `{V(u32)}` (no niche) | 4 | 4 | 8 | 8 | 8 | 8 |
| Full256 (256 unit variants, no niche) | 1 | 1 | 2 | 2 | - | 2 |

Ctx = `u8` (shows alignment padding; `(M,C)` is the baseline):

| M | M + C | (M,C) | Option | Slot | Mirror | Flag/uninit |
| --- | --- | --- | --- | --- | --- | --- |
| Unit | 2 | 2 | 2 | 2 | 2 | 3 |
| Strs | 25 | 32 | 32 | 32 | - | 32 |
| Owned | 25 | 32 | 32 | 32 | 32 | 32 |
| Pair | 3 | 3 | 3 | 3 | - | 4 |
| Player | 49 | 56 | 56 | 56 | 56 | 56 |
| Boxed | 9 | 16 | 24 | 24 | 24 | 16 |
| Only | 5 | 8 | 12 | 12 | 12 | 8 |
| Full256 | 2 | 2 | 3 | 3 | - | 3 |

Findings:

- **Free when `M` has niche values.** `Option<M>` needs one spare value, `Slot<M>` needs two (unstarted and poisoned). Every enum with at least two variants and a tag byte, or with a `String`/`Vec`/`&str` payload, qualifies. Unit-only, `Pair`, `Strs`, `Owned`, `Player`: overhead 0.
- **AC-15 cannot hold literally in two cases.**
  1. *Alignment padding, independent of poisoning.* A plain `struct { M, C }` already breaks `size_of M + size_of C` (`Unit` + `u64`: 9 vs 16; `Player` + `u8`: 49 vs 56). The sum is only exact when it is a multiple of the alignment, e.g. a ZST `Ctx`.
  2. *No usable niche in `M`.* Boxed-with-unit-variant (the unit variant eats the only niche of `Box`), single-variant enums over full-range integers, fieldless enums with 256 variants, and any `M` whose niches are exhausted. The poison/started information then costs at most `align_of::<M>()` bytes (`Boxed`: 8, `Only`: 4, `Full256`: 1), sometimes absorbed by `Ctx` padding.
- A `bool` flag (c, d) beats the niche slot only in case 2 with a padded `Ctx` (`Boxed`/`u8`: 16 vs 24) and loses by up to one alignment unit in the common case (`Unit`: 2 vs 1; `Player`: 56 vs 48). The common case wins; not worth `unsafe` or a placeholder.
- `Slot<M>` needs two niche values where `Option<M>` needs one: for a single-variant `enum { Data(Box<u32>) }` Option is 8 bytes but Slot is 16. Accepted: such roots are degenerate, and the unstarted state is required by 4.5 regardless.

Required spec changes (errata, to be added to `docs/spec-errata.md`; section 5 and 4.5 win over 3.5/3.6):

- **AC-15 and 3.5**: restate as "`size_of::<Machine<M>>() == size_of::<(M, M::Ctx)>()` for any machine without optional features whose root enum has at least two niche values (`size_of::<Slot<M>>() == size_of::<M>()`); otherwise the excess is at most `align_of::<M>()`". The test asserts the equality for the niche fixtures and the bound for the rest.
- **3.5**: the "logical content" has no room for the *started* bit that 4.5 needs (`new` runs no entries, `send` starts implicitly). `Slot::Unstarted` absorbs it for free; the struct in 3.5 should show `slot: Slot<M>` instead of `state: M`.
- **3.6**: replace the hidden `__Poisoned` variant with the `Slot` design; 13.1 is closed, the mirror enum and `Option<M>` are both rejected in favour of their combination. `state()` and `try_state()` also cover "not started" (spec silent: `state()` panics with a distinct message, `try_state()` returns `None`).
- **5.11**: "writes `__Poisoned`" becomes "writes `Slot::Poisoned`"; dropping the half-changed value happens during unwinding, so a `Drop` impl that panics there aborts.
- The macro cannot see niches, so it cannot warn about case 2; the size warning of 3.4 may reuse the same `size_of::<Slot<M>>() == size_of::<M>()` check in a generated test.
