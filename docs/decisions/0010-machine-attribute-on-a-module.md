# 0010. `#[machine]` goes on an inline module

Status: accepted
Date: 2026-11-12
Spec: 3.2, 6.1, 6.5, 6.6, principle 4

## Context

The spec puts `#[machine]` on the root enum, `#[state]` on each state struct or enum and `#[handlers]` on an `impl` block, all as separate items. An attribute proc-macro only sees the item it is applied to. Resolving a `goto!(T)` needs the whole tree (the path from the handler's state to `T`, the LCA, the exit and entry sequence, the data of intermediate states), and so does the graph. Separate items could only share that through type-level encodings (traits describing each node) or `macro_rules!` callbacks exported between items; both are fragile (declaration order, macro scoping), produce worse errors and slow compilation (BM-05).

## Decision

`#[estado::machine(event = .., effect = .., ctx = .., ...)]` is applied to an inline module that contains the whole machine:

```rust
#[estado::machine(event = Ev, effect = Cmd, ctx = Env)]
mod player {
    use super::*;

    #[root]
    pub enum Player { #[initial] Idle, Active(Active), Failed { reason: String } }

    #[state]
    pub struct Active { track: TrackId, buf: AudioBuffer, #[substates] sub: ActiveSub }

    #[state]
    pub enum ActiveSub { #[initial] Playing { pos: Duration }, Paused { pos: Duration } }

    #[handlers]
    impl Player {
        #[on(Playing)]
        fn playing(state: PlayingMut<'_>, up: ActiveFields<'_>, cx: &mut Cx<Self>, event: &Ev) -> Next<Self> { .. }
    }
}
pub use player::Player;
```

- The module's root enum is marked `#[root]`. Every other attribute of section 6.1 keeps its name and meaning inside the module; the machine options move from the enum to the module attribute.
- `#[state]`, `#[root]`, `#[handlers]` and the per-function attributes are consumed by `#[machine]` and removed from the output. They also exist as standalone attributes that report a clear error (E0001) when used outside a `#[machine]` module, so rust-analyzer resolves them.
- Generated items go into the module, in a nested `#[doc(hidden)] mod __estado` (spec 6.5), with the public views (`*Fields`, `*Mut`) and the `StateId` consts re-exported from the machine module.
- Items in the module that are not part of the machine (helpers, constants, other `impl`s) pass through unchanged.

## Consequences

- The macro sees the complete machine, so it can generate linear exit/entry code, resolve every `goto!` statically and report precise errors with spans.
- rust-analyzer and rustfmt keep working: the module body is ordinary Rust items with inert attributes.
- One extra level of nesting for the user, and a `pub use` to expose the machine. A machine split across files is not supported in v0 (`mod player;` without a body is error E0001 with a hint).
- E0001 changes meaning from "`#[machine]` outside an enum" to "`#[machine]` not on an inline module, or the module has no single `#[root]` enum" (spec errata 7).
