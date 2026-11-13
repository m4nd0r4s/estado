# Tasks

`#[invoke]` starts a background task for a state instance. The machine describes the task and your runtime runs it.

## `#[invoke]`

`#[invoke(task)]` starts a background task when the state is entered. `task` is a value of the machine's **effect type**: a command that your runtime knows how to run. The machine emits `Effect::Invoke { key, task }` on entry and `Effect::Cancel(key)` on exit.

When the work finishes, turn the result into an event and call `complete(key, event)`. The machine accepts it only if `key` belongs to the active instance. Otherwise `complete` returns `Ok` with `ignored = Some(IgnoreReason::Stale)` and drops the event.

The key is stable, so your runtime can use it as an idempotency key. `estado` does not promise to deliver a result exactly once.

```rust
use estado::prelude::*;
use estado::{Effect, IgnoreReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Load(u32),
    Loaded(String),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    FetchUser(u32),
}

#[estado::machine(event = Event, effect = Command)]
mod profile {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Profile {
        #[initial]
        Empty,
        #[invoke(|loading| Command::FetchUser(*loading.id))]
        Loading { id: u32 },
        Ready { name: String },
    }

    #[handlers]
    impl Profile {
        #[on(Empty)]
        fn on_empty(event: &Event) -> Next<Self> {
            match event {
                Event::Load(id) => goto!(Loading { id: *id }),
                _ => unhandled!(),
            }
        }

        #[on(Loading)]
        fn on_loading(event: &Event) -> Next<Self> {
            match event {
                Event::Loaded(name) => goto!(Ready { name: name.clone() }),
                Event::Cancel => goto!(Empty),
                _ => unhandled!(),
            }
        }

        #[on(Ready)]
        fn on_ready(_ready: ReadyMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Empty)
        }
    }
}

use profile::Profile;

fn main() {
    let mut machine = Machine::<Profile>::new(());

    // Entering `Loading` starts the task.
    let step = machine.send(Event::Load(7)).unwrap();
    let effects: Vec<_> = step.effects.into_iter().collect();
    let [Effect::Invoke { key, task }] = &effects[..] else {
        panic!("one invoke expected");
    };
    assert_eq!(*task, Command::FetchUser(7));

    // The runtime finished the work and reports it.
    machine.complete(*key, Event::Loaded("Ada".to_string())).unwrap();
    assert_eq!(*machine.state(), Profile::Ready { name: "Ada".to_string() });

    // A late result for the same key is stale.
    let report = machine.complete(*key, Event::Loaded("Eve".to_string())).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(*machine.state(), Profile::Ready { name: "Ada".to_string() });
}
```

Leaving `Loading` before the task ends emits `Effect::Cancel(key)`:

```rust
use estado::prelude::*;
use estado::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Load,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Download,
}

#[estado::machine(event = Event, effect = Command)]
mod fetch {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Fetch {
        #[initial]
        Idle,
        #[invoke(Command::Download)]
        Busy,
    }

    #[handlers]
    impl Fetch {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Load => goto!(Busy),
                _ => unhandled!(),
            }
        }

        #[on(Busy)]
        fn on_busy(event: &Event) -> Next<Self> {
            match event {
                Event::Abort => goto!(Idle),
                _ => unhandled!(),
            }
        }
    }
}

use fetch::Fetch;

fn main() {
    let mut machine = Machine::<Fetch>::new(());
    let started: Vec<_> = machine.send(Event::Load).unwrap().effects.into_iter().collect();
    let [Effect::Invoke { key: started_key, .. }] = &started[..] else {
        panic!("one invoke expected");
    };

    let stopped: Vec<_> = machine.send(Event::Abort).unwrap().effects.into_iter().collect();
    assert!(matches!(&stopped[..], [Effect::Cancel(key)] if key == started_key));
}
```

A timer and a task of the same state instance share one epoch. An expression that is not a closure, such as `Command::Download` above, is used as it is.

## How a runtime feeds timers and tasks

A runtime reads the effects of every step and acts on them:

| Effect | The runtime |
| --- | --- |
| `StartTimer { key, after }` | arranges to call `machine.fire(key)` once `after` has passed |
| `CancelTimer(key)` | may forget the timer. Firing it later is harmless. |
| `Invoke { key, task }` | runs `task`, then calls `machine.complete(key, event)` with the result |
| `Cancel(key)` | may stop the task. A result that still arrives is ignored. |
| `User(command)` | does whatever the command means |

The [`Driver`](./12-sinks-and-drivers.md) trait names those five actions. The Tokio and Embassy runtimes that implement them are not available yet. For tests, [the Harness](./19-testing.md) plays the runtime for you, with a virtual clock.

## Next

[History](./17-history.md) lets a compound state remember where it was.
