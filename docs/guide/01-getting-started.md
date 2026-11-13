# Getting started

This page installs `estado`, builds a first machine and shows how to drive it. The next pages explain each part in detail.

## Install

`estado` is not on crates.io yet. Add it as a git dependency:

```toml
[dependencies]
estado = { git = "https://github.com/m4nd0r4s/estado" }
```

The crate needs Rust 1.85 or later (edition 2024). Most of this guide comes from `estado::prelude::*`. A few types that some samples use, such as `IgnoreReason`, `FnSink`, `TimerKey`, `InvokeKey`, `Overflow`, `Driver`, `EdgeTarget` and `StateKind`, are not in the prelude. The samples import them from `estado::`, for example `use estado::{IgnoreReason, TimerKey};`.

## What you write

A machine is one inline module marked `#[estado::machine(...)]`. It holds everything the macro needs to see at once:

- one `#[root]` enum, the top level of the state tree;
- optional `#[state]` types for states that have substates or regions;
- one `#[handlers]` block with the reactions to events.

The attribute names the **event** type the machine reacts to. It can also name an **effect** type (commands the machine asks the outside world to run) and a **context** type (shared data). Both are optional. The [context and effects](./11-context-and-effects.md) page covers them.

The macro lives on a module because it must see the whole tree to check every transition at compile time. Items in the module that are not part of the machine, such as helper functions, pass through unchanged.

## A first machine

This turnstile has two states. A coin unlocks it, a push locks it again. Breaking it ends the machine.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Coin,
    Push,
    Break,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Unlock,
    Lock,
}

#[estado::machine(event = Event, effect = Command)]
mod turnstile {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Turnstile {
        #[initial]
        Locked,
        Unlocked,
        #[terminal]
        Broken,
    }

    #[handlers]
    impl Turnstile {
        #[on(Locked)]
        fn on_locked(event: &Event) -> Next<Self> {
            match event {
                Event::Coin => goto!(Unlocked),
                Event::Break => goto!(Broken),
                _ => unhandled!(),
            }
        }

        #[on(Unlocked)]
        fn on_unlocked(event: &Event) -> Next<Self> {
            match event {
                Event::Push => goto!(Locked),
                Event::Break => goto!(Broken),
                _ => unhandled!(),
            }
        }

        #[entry(Unlocked)]
        fn enter_unlocked(cx: &mut Cx<Self>) {
            cx.emit(Command::Unlock);
        }

        #[entry(Locked)]
        fn enter_locked(cx: &mut Cx<Self>) {
            cx.emit(Command::Lock);
        }
    }
}

use turnstile::Turnstile;

fn main() {
    let mut machine = Machine::<Turnstile>::new(());

    // The first `send` starts the machine: the entry of `Locked` runs first.
    let step = machine.send(Event::Coin).unwrap();
    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::Lock, &Command::Unlock]
    );
    assert_eq!(machine.path().to_string(), "Unlocked");
    assert_eq!(*machine.state(), Turnstile::Unlocked);
    assert!(!machine.is_done());

    machine.send(Event::Break).unwrap();
    assert!(machine.is_done());
}
```

Read it from the top:

- `#[root]` marks the enum that is the machine. Its `#[initial]` variant is where the machine starts. Variants marked `#[terminal]` end the machine.
- Each `#[on(State)]` function handles the events of one state. It receives the event by reference and returns a `Next<Self>`.
- `goto!(Unlocked)` moves to another state. `unhandled!()` says this state does not react to the event.
- `#[entry(State)]` runs when a state is entered. `cx.emit` records an effect. Entries and exits are covered in [Entry and exit](./08-entry-exit.md).

The `use turnstile::Turnstile;` line brings the root type out of the module so you can name it.

## Driving the machine

`Machine<M>` owns the root state, the context and a small runtime area. You use five calls most of the time.

| Call | What it does |
| --- | --- |
| `Machine::new(ctx)` | Builds the machine. It runs no entry yet. Pass `()` when the machine has no context. |
| `send(event)` | Processes one event to completion and returns `Result<Step, SendError>`. |
| `path()` | The active states as a `StatePath`. `to_string()` gives `Unlocked`, or `Active/Paused` for nested states. |
| `state()` | The root enum, with all the data of the active states. |
| `is_done()` | `true` once the root reached a `#[terminal]` state. |

`Machine::new` is a `const fn` and does not run the initial entries. The first `send` starts the machine for you: it runs the entries of the initial configuration, then the event. To run them earlier, for example to collect the first effects, call `start()`:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Toggle,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    LampOff,
}

#[estado::machine(event = Event, effect = Command)]
mod lamp {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lamp {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Lamp {
        #[entry(Off)]
        fn enter_off(cx: &mut Cx<Self>) {
            cx.emit(Command::LampOff);
        }

        #[on(Off)]
        fn on_off(_event: &Event) -> Next<Self> {
            goto!(On)
        }

        #[on(On)]
        fn on_on(_event: &Event) -> Next<Self> {
            goto!(Off)
        }
    }
}

use lamp::Lamp;

fn main() {
    let mut machine = Machine::<Lamp>::new(());
    assert!(machine.try_state().is_none()); // not started: `state()` would panic

    let step = machine.start().unwrap();
    assert_eq!(step.effects.commands().collect::<Vec<_>>(), [&Command::LampOff]);
    assert_eq!(*machine.state(), Lamp::Off);
}
```

`state()` panics when the machine has not started or is poisoned. `try_state()` returns an `Option` instead.

## What `send` returns

`send` returns a `Step` with two fields:

- `effects` holds the effects of the whole run, in emission order. Call `commands()` on it to see only the ones you emitted with `cx.emit`. Timers and tasks show up there too, as [`Effect`](./11-context-and-effects.md) values you can `iter()` over.
- `report` counts what happened: `handled`, `transitions`, `microsteps`, `unhandled_internal` and `ignored`.

A `send` is **run to completion**. It handles the event you gave it and every consequence of it (raised events, automatic transitions, redelivered deferred events) before it returns.

An event nobody handles is not an error by default. `send` returns `Ok` and the report says it was ignored:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Known,
    Unknown,
}

#[estado::machine(event = Event)]
mod door {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Door {
        #[initial]
        Closed,
        Open,
    }

    #[handlers]
    impl Door {
        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                Event::Known => goto!(Open),
                _ => unhandled!(),
            }
        }

        #[on(Open)]
        fn on_open(_event: &Event) -> Next<Self> {
            goto!(Closed)
        }
    }
}

use door::Door;

fn main() {
    let mut machine = Machine::<Door>::new(());

    let report = machine.send(Event::Unknown).unwrap().report;
    assert!(!report.handled);
    assert_eq!(report.ignored, Some(estado::IgnoreReason::Unhandled));

    let report = machine.send(Event::Known).unwrap().report;
    assert!(report.handled);
    assert_eq!(report.transitions, 1);
}
```

To turn unhandled events into errors, set `unhandled = error` on the machine. See [Unhandled events](./07-unhandled-events.md).

## Methods at a glance

The calls below are all on `Machine<M>`. Each one is explained on the page named in the last column. The [machine reference](../reference/machine.md) has the exact signatures.

| Call | Purpose | Page |
| --- | --- | --- |
| `new(ctx)`, `start()` | Build and start | this page |
| `send(event)` | Process an event | this page |
| `send_into`, `send_with`, `send_batch` | Process events into your own sink | [Sinks and drivers](./12-sinks-and-drivers.md) |
| `fire(key)`, `complete(key, event)` | Feed timers and task results | [Timers](./15-timers.md) and [Tasks](./16-tasks.md) |
| `state()`, `try_state()`, `path()`, `is_in(id)` | Look at the configuration | [States](./02-states.md) |
| `ctx()`, `ctx_mut()` | The global context | [Context and effects](./11-context-and-effects.md) |
| `is_done()`, `take_output()` | A finished machine | [States](./02-states.md#terminal-states) |
| `is_poisoned()` | A broken machine | [Errors and safety](./21-errors-and-safety.md) |

## What is not here yet

Graph export, the Tokio and Embassy runtimes, serde snapshots and the typestate mode belong to later milestones (M4 to M6) and are not part of this guide.

## Next

[States](./02-states.md) explains the state tree: compound states, initial states and terminal states.
