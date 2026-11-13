# Always and done

`#[always]` and `#[on_done]` run without an event.

## `#[always]`

`#[always(State)]` is a handler that reacts to the state's own entry. It runs as the **next microstep** right after the entries, before any raised or redelivered event. It has no event parameter. It returns `Next<Self>`, and only two answers make sense:

- `stay!()` ends it;
- `goto!(..)` starts a transition. Use `goto!(.., external)` to re-enter the state itself.

`reenter!()`, `defer!()`, `bubble!()` and `unhandled!()` there are [E0045](../errors/E0045.md).

An `#[always]` runs **once per entry**, not after every later event. It counts toward the livelock budget (`max_microsteps`), so a loop that never settles ends in `SendError::Livelock`. See [Errors and safety](./21-errors-and-safety.md).

The next machine retries a check up to three times with no event at all.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
}

#[estado::machine(event = Event)]
mod retry {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Retry {
        #[initial]
        Idle,
        Checking { tries: u8 },
        Settled { tries: u8 },
    }

    #[handlers]
    impl Retry {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Checking { tries: 1 })
        }

        // Runs right after every entry of `Checking`.
        #[always(Checking)]
        fn check(checking: CheckingMut<'_>) -> Next<Self> {
            if *checking.tries >= 3 {
                goto!(Settled { tries: *checking.tries })
            } else {
                goto!(Checking { tries: *checking.tries + 1 }, external)
            }
        }

        #[on(Settled)]
        fn on_settled(_settled: SettledMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use retry::Retry;

fn main() {
    let mut machine = Machine::<Retry>::new(());
    let step = machine.send(Event::Start).unwrap();

    assert_eq!(*machine.state(), Retry::Settled { tries: 3 });
    // One microstep for the event, three for the `#[always]` evaluations.
    assert_eq!(step.report.microsteps, 4);
}
```

### `#[always]` runs before raised events

The evaluation is the next microstep after the entry. It comes before events raised with `cx.raise`. The log shows the order:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Note(u8),
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod probe {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Probe {
        #[initial]
        Start,
        Checking,
        Settled,
    }

    #[handlers]
    impl Probe {
        #[on(Start)]
        fn on_start(_event: &Event) -> Next<Self> {
            goto!(Checking)
        }

        #[entry(Checking)]
        fn enter_checking(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Checking".to_string());
            cx.raise(Event::Note(9));
        }

        // The next microstep after the entry, before the raised `Note(9)`.
        #[always(Checking)]
        fn always_checking(cx: &mut Cx<Self>) -> Next<Self> {
            cx.ctx_mut().push("always Checking".to_string());
            goto!(Settled)
        }

        #[entry(Settled)]
        fn enter_settled(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Settled".to_string());
        }

        #[on(Settled)]
        fn on_settled(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push(format!("Settled sees {event:?}"));
            match event {
                Event::Note(_) => stay!(),
                Event::Go => goto!(Start),
            }
        }
    }
}

use probe::Probe;

fn main() {
    let mut machine = Machine::<Probe>::new(Vec::new());
    machine.start().unwrap();

    machine.send(Event::Go).unwrap();
    assert_eq!(
        machine.ctx(),
        &["enter Checking", "always Checking", "enter Settled", "Settled sees Note(9)"]
    );
}
```

### `#[always]` runs once per entry

An `#[always]` that answers `stay!()` does not run again on later events. It runs again only when the state is entered again:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Poke,
    Leave,
    Return,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod guard {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Guard {
        #[initial]
        Away,
        Watching,
    }

    #[handlers]
    impl Guard {
        #[on(Away)]
        fn on_away(event: &Event) -> Next<Self> {
            match event {
                Event::Return => goto!(Watching),
                _ => unhandled!(),
            }
        }

        #[always(Watching)]
        fn look(cx: &mut Cx<Self>) -> Next<Self> {
            cx.ctx_mut().push("looked".to_string());
            stay!()
        }

        #[on(Watching)]
        fn on_watching(event: &Event) -> Next<Self> {
            match event {
                Event::Poke => stay!(),
                Event::Leave => goto!(Away),
                Event::Return => unhandled!(),
            }
        }
    }
}

use guard::Guard;

fn main() {
    let mut machine = Machine::<Guard>::new(Vec::new());
    machine.send(Event::Return).unwrap();
    machine.send(Event::Poke).unwrap();
    machine.send(Event::Poke).unwrap();
    assert_eq!(machine.ctx(), &["looked"]); // once

    machine.send(Event::Leave).unwrap();
    machine.send(Event::Return).unwrap();
    assert_eq!(machine.ctx(), &["looked", "looked"]); // once more, for the new entry
}
```

### Order of pending evaluations

Each microstep serves one pending evaluation. The completion checks of [`#[on_done]`](./10-on-done.md) run **before** any `#[always]`. When several `#[always]` are pending at once, the deepest state goes first, so an inner state is evaluated before the compound state that holds it.

## Next

[On done](./10-on-done.md) covers the reaction to a finished compound state.
