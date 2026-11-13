# Configuration

This page covers how to depend on `estado`: the crates, the Cargo features, what the facade and the prelude export, the `no_std` build, the supported Rust version and a build-time tip.

## Install

`estado` is not on crates.io yet. Use a git dependency:

```toml
[dependencies]
estado = { git = "https://github.com/m4nd0r4s/estado" }

[dev-dependencies]
estado-test = { git = "https://github.com/m4nd0r4s/estado" }
```

Depend on the `estado` facade. It re-exports `estado-core` and the macros, and the code that `#[machine]` generates finds `estado-core` through it. You do not need `estado-core` or `estado-macros` in your `Cargo.toml`.

## Crates

| Crate | Contents | Status |
| --- | --- | --- |
| `estado` | Facade: re-exports `estado-core` and the macros, plus the prelude | usable |
| `estado-core` | `Spec`, `Machine`, `Cx`, effects, sinks, graph types. `no_std`, no `alloc` needed, no dependencies | usable |
| `estado-macros` | `#[machine]`, `#[root]`, `#[state]`, `#[handlers]` and the markers they read | usable |
| `estado-test` | `Harness`, virtual clock, `assert_state!` | basic. See [testing](testing.md) |
| `estado-graph` | Mermaid, DOT, SCXML and JSON export, `assert_snapshot!` (analyses come later) | export usable. See [graph](graph.md) |
| `estado-serde` | Versioned snapshot, restore and migration | not implemented yet (empty crate) |
| `estado-tokio` | Tokio runtime: actor, timers, tasks | not implemented yet (empty crate) |
| `estado-embassy` | Embassy runtime for `no_std` | not implemented yet (empty crate) |
| `estado-inspect` | Live inspector, trace recording, replay | not implemented yet (empty crate) |

Until a runtime crate exists, you write the loop that executes effects yourself. See [effects](effects.md#feeding-results-back); it is a handful of lines around `Driver`, `Machine::fire` and `Machine::complete`.

## Features

The features of `estado` forward to `estado-core`. Defaults: `std`.

| Feature | Default | Effect today |
| --- | --- | --- |
| `std` | yes | Implies `alloc`. Adds `impl std::error::Error for SendError<Event>` (when `Event: Debug`). |
| `alloc` | via `std` | `InlineEffects` spills extra effects to the heap instead of failing with `Overflow`. `Vec<Effect<Command>>` becomes a `Sink`. |
| `tracing` | no | Reserved for the upcoming `estado-inspect` crate (trace recording). Nothing reads it yet. |
| `inspect` | no | Reserved for the upcoming `estado-inspect` crate (live inspector). Nothing reads it yet. |
| `invariants` | no | Reserved for `#[invariant]`, which is not supported yet ([`E0090`](../errors/E0090.md)). Nothing reads it yet. |
| `serde` | no | Reserved for the upcoming `estado-serde` crate (snapshots). Nothing reads it yet. |

The four reserved features are declared so that `Cargo.toml` files written today keep working, but they change nothing in the code that is generated or compiled. Enabling one is harmless. Each gains meaning when the crate that needs it lands.

## no_std

The facade and the core are `#![no_std]`. Turn the default features off:

```toml
[dependencies]
estado = { git = "https://github.com/m4nd0r4s/estado", default-features = false }
```

- **Without `alloc`**: no heap is used by the library. The effect buffer of a step holds `fx_inline` effects (default 4), and a step that emits more fails with `SendError::EffectOverflow`. Size `fx_inline` for the largest burst, or use `send_into` with a sink of your own. The raise and defer queues are fixed-size (`raise_cap`, `defer_cap`).
- **With `alloc`** (and no `std`): effects spill to the heap beyond `fx_inline`.
- The generated code uses `::core` paths and fixed-size queues. Your own types may allocate.
- `estado-core` builds for a bare-metal target (`thumbv7em-none-eabihf`) in CI.

## Rust version

The minimum supported Rust version is 1.85 (edition 2024). `estado-core` is `#![forbid(unsafe_code)]`.

## Build time

`#[machine]` is a procedural macro. It runs at compile time, and in a dev build it is not optimized, so a machine with many states can take noticeably longer to compile. Compile the macro with optimizations in dev builds:

```toml
[profile.dev.package.estado-macros]
opt-level = 3
```

## How the generated code finds estado-core

The macro writes paths to `estado-core` into your code. It finds the crate from your `Cargo.toml`, in this order:

1. through the `estado` facade, under whatever name you gave it;
2. else through `estado-core`, under whatever name;
3. else as `::estado_core`.

Limitation: the macro cannot tell a normal dependency from a dev-dependency. A crate that has `estado-core` and `estado-macros` as normal dependencies and `estado` only as a dev-dependency resolves to the facade, which its library build does not have, and fails to compile. Give such a crate the facade as a normal dependency.

## The facade

`estado` re-exports everything public in `estado-core` at its root, and the four attribute macros:

```rust,ignore
pub use estado_core::*;
pub use estado_macros::{handlers, machine, root, state};
```

The transition macros are `macro_rules!` items exported at the crate root: `estado::goto!`, `estado::stay!`, `estado::bubble!`, `estado::unhandled!`, `estado::defer!`, `estado::reenter!`, `estado::goto_dyn!`, `estado::edges!` and `estado::Exiting!`. They are stubs that report [`E0040`](../errors/E0040.md) when used outside a machine; see [macros](macros.md).

## The prelude

`use estado::prelude::*;` brings in:

| Group | Names |
| --- | --- |
| Types | `Cx`, `Discard`, `Edge`, `Effect`, `Machine`, `Next`, `Report`, `SendError`, `Sink`, `Spec`, `StateId`, `Step` |
| Attribute macros | `handlers`, `machine`, `root`, `state` |
| Transition macros | `goto`, `stay`, `bubble`, `unhandled`, `defer`, `reenter`, `goto_dyn`, `edges`, `Exiting` |

Everything else is one path away, at the crate root: `estado::Driver`, `estado::DriverSink`, `estado::FnSink`, `estado::InlineEffects`, `estado::Overflow`, `estado::TimerKey`, `estado::InvokeKey`, `estado::IgnoreReason`, `estado::StatePath`, `estado::Graph`, `estado::UnhandledPolicy`, and so on. The whole list is on the [core](core.md#public-items) page.

The attributes `#[initial]`, `#[terminal]`, `#[substates]`, `#[region]`, `#[history]`, `#[after]`, `#[invoke]`, `#[unhandled]`, `#[on]`, `#[entry]`, `#[exit]`, `#[always]` and `#[on_done]` are not imports. They are markers inside a `#[machine]` module and need no `use`.

```rust
use estado::prelude::*;
use estado::{InlineEffects, StatePath}; // not in the prelude

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
}

#[estado::machine(event = Event, allow(W0002))]
mod light {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Light {
        #[on(Off)]
        fn on_off(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(On),
            }
        }
    }
}

use light::Light;

fn main() {
    let mut machine = Machine::<Light>::new(());
    let step = machine.send(Event::Go).unwrap();

    let effects: &InlineEffects<(), 4> = &step.effects;
    assert!(effects.is_empty());

    let path: StatePath = machine.path();
    assert_eq!(path.to_string(), "On");
}
```

## What the macro needs from your types

- The module can be private, and the root enum and state types can have any visibility. Generated types (`*Fields`, `*Mut`, `*Exiting`) take the visibility of the type they come from.
- `#[derive]` and other attributes on your types are kept. The macro requires no trait on the event, the effect or the states. Some tools do: `estado-test` needs `Debug` on the event and effect type, and `assert_eq!` on a state needs the usual derives.
- Use `#[estado::machine(..)]` or, after the prelude import, `#[machine(..)]`. The marker attributes can be written plain (`#[initial]`) or qualified (`#[estado::initial]`).
