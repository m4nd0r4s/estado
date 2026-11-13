# Testing tasks and errors

This page continues the [harness](./19-testing.md): tasks, stale results and errors.

## Tasks

A state with `#[invoke]` puts its task in the pending set. Complete it with `complete_invoke`, giving a predicate that finds the task and the event the result becomes. Without a match, the harness panics and lists the pending tasks.

```rust
use estado::prelude::*;
use estado_test::{Harness, assert_state};

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
    let mut harness = Harness::new(Machine::<Profile>::new(()));
    harness.send(Event::Load(7));
    assert_eq!(harness.pending_invokes().len(), 1);
    assert_eq!(harness.pending_invokes()[0].task, Command::FetchUser(7));

    harness.complete_invoke(
        |task| matches!(task, Command::FetchUser(7)),
        Event::Loaded("Ada".to_string()),
    );
    assert_state!(harness, Profile::Ready { .. });
    assert!(harness.pending_invokes().is_empty());

    // Leaving a state drops its task from the pending set.
    harness.send(Event::Cancel);
    harness.send(Event::Load(8));
    let stale_key = harness.pending_invokes()[0].key;
    harness.send(Event::Cancel);
    assert!(harness.pending_invokes().is_empty());

    // A result for a task that is gone is ignored, not delivered.
    harness.complete_key(stale_key, Event::Loaded("Eve".to_string()));
    assert_state!(harness, Profile::Empty);
}
```

## Testing errors

The harness panics on a `SendError`, which is what you want for the happy path. To check an error, go through `machine_mut()`, which bypasses the harness:

```rust
use estado::prelude::*;
use estado_test::Harness;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Noise,
}

#[estado::machine(event = Event, unhandled = error)]
mod strict {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Strict {
        #[initial]
        Waiting,
        Going,
    }

    #[handlers]
    impl Strict {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Going),
                _ => unhandled!(),
            }
        }

        #[on(Going)]
        fn on_going(_event: &Event) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

use strict::Strict;

fn main() {
    let mut harness = Harness::new(Machine::<Strict>::new(()));
    let result = harness.machine_mut().send(Event::Noise);
    assert!(matches!(result, Err(SendError::Unhandled(Event::Noise))));
}
```

## Plain machines need no harness

You do not always need the harness. A machine without timers or tasks is a plain value: send events to a `Machine` and assert on `path()`, `state()`, the effects and the context, as the other pages of this guide do. Use the harness when time or tasks matter.

## Next

[Errors and safety](./21-errors-and-safety.md) lists every way `send` can fail.
