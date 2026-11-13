# On done

`#[on_done]` reacts to a compound or parallel state that has finished.

## `#[on_done]`

A `#[terminal]` substate completes its parent. When the active substate of a compound state enters a terminal, the machine runs one extra microstep that calls `#[on_done(Parent)]`.

The handler receives the parent's `*Fields`, then the payload of the terminal state if it is a newtype variant, then `cx`. A struct-like or tuple terminal gives the handler no payload. It returns `Next<Self>` with the same limits as `#[always]`: only `stay!()` and `goto!`. A compound state without `#[on_done]` just stays in its terminal substate.

`#[on_done]` on a state that cannot complete is [E0044](../errors/E0044.md).

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Begin,
    Received(u32),
}

#[estado::machine(event = Event, ctx = Vec<&'static str>)]
mod download {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Download {
        #[initial]
        Idle,
        Working(Working),
        Done { url: String, bytes: u32 },
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Working {
        pub url: String,
        #[substates]
        pub phase: Phase,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Phase {
        #[initial]
        Fetching,
        #[terminal]
        Fetched(u32),
    }

    #[handlers]
    impl Download {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Working { url: "https://example.org".to_string() })
        }

        #[on(Fetching)]
        fn on_fetching(event: &Event) -> Next<Self> {
            match event {
                Event::Received(bytes) => goto!(Fetched(*bytes)),
                _ => bubble!(),
            }
        }

        #[entry(Fetched)]
        fn enter_fetched(_bytes: &mut u32, cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Fetched");
        }

        // The microstep after `Fetched` is entered.
        #[on_done(Working)]
        fn working_done(working: WorkingFields<'_>, bytes: &u32, cx: &mut Cx<Self>) -> Next<Self> {
            cx.ctx_mut().push("working done");
            let url = working.url.clone();
            goto!(Done { url, bytes: *bytes })
        }

        #[on(Done)]
        fn on_done_state(_done: DoneMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use download::Download;

fn main() {
    let mut machine = Machine::<Download>::new(Vec::new());
    machine.send(Event::Begin).unwrap();
    let step = machine.send(Event::Received(512)).unwrap();

    assert_eq!(machine.ctx(), &["enter Fetched", "working done"]);
    assert_eq!(
        *machine.state(),
        Download::Done { url: "https://example.org".to_string(), bytes: 512 }
    );
    // The event, then the completion.
    assert_eq!(step.report.microsteps, 2);
}
```

### Without a payload, and without `#[on_done]`

A `#[terminal]` substate without data gives the handler nothing to receive: skip the payload parameter. A compound state with no `#[on_done]` handler simply stays in its terminal substate, and its own handlers keep working. The completion still costs a microstep that does nothing.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Finish,
    Reset,
    Quit,
}

#[estado::machine(event = Event)]
mod task {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Task {
        #[initial]
        Waiting,
        // With `#[on_done]`, no payload.
        Chained(Chained),
        // Without `#[on_done]`: stays in `Flushed`.
        Draining(Draining),
        Quit,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Chained {
        #[substates]
        pub step: ChainedStep,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum ChainedStep {
        #[initial]
        Working,
        #[terminal]
        Over,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Draining {
        #[substates]
        pub phase: Phase,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Phase {
        #[initial]
        Flushing,
        #[terminal]
        Flushed,
    }

    #[handlers]
    impl Task {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Finish => goto!(Chained),
                Event::Reset => goto!(Draining),
                Event::Quit => goto!(Quit),
            }
        }

        #[on(Working)]
        fn on_working(_event: &Event) -> Next<Self> {
            goto!(Over)
        }

        // No parameter for the payload: `Over` carries none.
        #[on_done(Chained)]
        fn chained_done() -> Next<Self> {
            goto!(Waiting)
        }

        #[on(Flushing)]
        fn on_flushing(_event: &Event) -> Next<Self> {
            goto!(Flushed)
        }

        // The terminal substate has no handler: the compound state's handler runs.
        #[on(Draining)]
        fn on_draining(_draining: DrainingFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Quit => goto!(Quit),
                _ => unhandled!(),
            }
        }

        #[on(Quit)]
        fn on_quit(_event: &Event) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

use task::Task;

fn main() {
    // `on_done` sends the machine back to `Waiting` within the same `send`.
    let mut chained = Machine::<Task>::new(());
    chained.send(Event::Finish).unwrap();
    assert_eq!(chained.path().to_string(), "Chained/Working");
    let report = chained.send(Event::Finish).unwrap().report;
    assert_eq!(chained.path().to_string(), "Waiting");
    assert_eq!(report.microsteps, 2);

    // Without `on_done` the machine rests in the terminal substate.
    let mut draining = Machine::<Task>::new(());
    draining.send(Event::Reset).unwrap();
    draining.send(Event::Finish).unwrap();
    assert_eq!(draining.path().to_string(), "Draining/Flushed");
    assert!(!draining.is_done()); // only a root `#[terminal]` ends the machine
    draining.send(Event::Quit).unwrap();
    assert_eq!(draining.path().to_string(), "Quit");
}
```

A parallel state completes when every region is in a `#[terminal]` state. Its `#[on_done]` gets the state's `*Fields` and no payload. See [Regions](./13-regions.md).

At the root, a `#[terminal]` does not run `#[on_done]`. It makes `is_done()` true instead ([States](./02-states.md#terminal-states)).

## Next

[Context and effects](./11-context-and-effects.md) covers `Cx`, the global context and the effects a machine produces.
