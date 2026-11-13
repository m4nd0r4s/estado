# estado

Hierarchical statecharts for Rust: handlers in idiomatic Rust, an exact static graph and an optional typestate mode, at zero cost when a feature is not used.

Documentation: **[m4nd0r4s.github.io/estado](https://m4nd0r4s.github.io/estado/)**: a guide that walks through every feature, the API reference and the error index.

> Status: v0 in development (milestone M3 of [`docs/spec.md`](docs/spec.md)). The macro, the core and the test harness work; graph export, runtimes, persistence and typestate come in M4-M6. Expect breaking changes.

## Quick start

```toml
[dependencies]
estado = { git = "https://github.com/m4nd0r4s/estado" }
```

A machine is an inline module marked `#[machine]`: one `#[root]` enum, `#[state]` types for compound and parallel states, and `#[handlers]` with the reactions. Handlers return `goto!`, `stay!`, `bubble!`, `defer!` or `reenter!`; the macro checks every transition at compile time and generates the dispatch, the exit/entry sequence and the graph.

```rust
use core::time::Duration;
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play(u32),
    Pause,
    Resume,
    Tick,
    Fail(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    OpenTrack(u32),
    StopOutput,
}

#[estado::machine(event = Event, effect = Command)]
mod player {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        #[terminal]
        Failed { reason: String },
    }

    /// A compound state: shared data plus its substates.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub track: u32,
        #[substates]
        pub sub: ActiveSub,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum ActiveSub {
        #[initial]
        Playing { position: Duration },
        Paused { position: Duration },
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                // Path form: data for `Active` and for the substate it starts in.
                Event::Play(track) => goto!(Active { track: *track } / Playing { position: Duration::ZERO }),
                _ => unhandled!(),
            }
        }

        #[entry(Active)]
        fn enter_active(active: ActiveFields<'_>, cx: &mut Cx<Self>) {
            cx.emit(Command::OpenTrack(*active.track));
        }

        #[exit(Active)]
        fn exit_active(cx: &mut Cx<Self>) {
            cx.emit(Command::StopOutput);
        }

        /// Handlers of a substate can read and write the parent's fields.
        #[on(Playing)]
        fn on_playing(playing: PlayingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Tick => {
                    *playing.position += Duration::from_secs(1);
                    stay!()
                }
                Event::Pause => goto!(Paused { position: *playing.position }),
                _ => bubble!(),
            }
        }

        #[on(Paused)]
        fn on_paused(paused: PausedMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Resume => goto!(Playing { position: *paused.position }),
                _ => bubble!(),
            }
        }

        /// Any substate of `Active` that does not handle `Fail` lands here.
        #[on(Active)]
        fn on_active(event: &Event) -> Next<Self> {
            match event {
                Event::Fail(reason) => goto!(Failed { reason: reason.clone() }),
                _ => unhandled!(),
            }
        }
    }
}

use player::{Active, ActiveSub, Player};

fn main() {
    let mut machine = Machine::<Player>::new(());
    let step = machine.send(Event::Play(7)).unwrap();
    assert_eq!(step.effects.commands().collect::<Vec<_>>(), [&Command::OpenTrack(7)]);

    machine.send(Event::Tick).unwrap();
    machine.send(Event::Pause).unwrap();
    assert_eq!(machine.path().to_string(), "Active/Paused");
    assert_eq!(
        *machine.state(),
        Player::Active(Active {
            track: 7,
            sub: ActiveSub::Paused { position: Duration::from_secs(1) },
        })
    );

    machine.send(Event::Fail("disk".into())).unwrap();
    assert!(machine.is_done());
}
```

More complete machines, each written both by hand against `estado-core` and with the macro, are in [`examples/`](examples): an HTTP/1 parser (A), a connection with retry, backoff and parallel keepalive (B), a game character with three regions (D) and a media player (I). Timers (`#[after]`), tasks (`#[invoke]`), history, `defer!()`, `cx.raise` and `#[always]` are covered there; [`estado-test`](crates/estado-test) drives them with a virtual clock.

## What the macro guarantees

- Every `goto!` target is checked at compile time; errors carry stable codes documented in [`docs/errors/`](docs/errors).
- Exits run deepest-first and entries outermost-first, with regions in declaration order (reverse on exit), as specified in section 5 of the spec.
- A panic during a transition poisons the machine; later sends return `SendError::Poisoned`.
- The core is `no_std` without allocation and has no dependencies.

Decisions and deviations from the spec are recorded in [`docs/decisions/`](docs/decisions) and [`docs/spec-errata.md`](docs/spec-errata.md).

## Crates

| Crate | Contents | Status |
| --- | --- | --- |
| `estado` | Facade and prelude | usable |
| `estado-core` | Traits, step algorithm, effects, graph types (`no_std`, no alloc) | usable |
| `estado-macros` | `#[machine]`, `#[state]`, `#[handlers]`, `goto!` | usable |
| `estado-test` | Harness with a virtual clock | basic |
| `estado-graph` | Mermaid/DOT/SCXML/JSON export and snapshot assertions (analyses in M4) | export usable |
| `estado-serde` | Versioned snapshot and restore | M6 |
| `estado-tokio` / `estado-embassy` | Runtimes | M5 |
| `estado-inspect` | Live inspector and replay | M6 |

For large machines, compile the macro with optimizations in dev builds:

```toml
[profile.dev.package.estado-macros]
opt-level = 3
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
