# Defer and raise

Two queues let events wait or flow inside the machine:

- `defer!()` keeps an event for later, when the machine can handle it.
- `cx.raise(event)` sends an event from inside the machine to itself.

`defer!()` stores the event, so it needs an event it can own.

## The order of work in one `send`

A `send` is run to completion. After the external event, the machine keeps going until nothing is left, in this order:

1. a pending `#[always]` evaluation;
2. **raised** events, first in, first out;
3. **redelivered deferred** events.

The immediate consequences of a transition always come before the deferred events.

## `defer!()`

A handler that is not ready for an event can return `defer!()`. The machine stores the event. Whenever the **active configuration changes**, every deferred event is redelivered, in the order it was deferred. A redelivered event can be deferred again.

While the configuration stays the same, nothing is redelivered. The next example logs what each state sees.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Work(u8),
    Start,
    Done,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod worker {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Worker {
        #[initial]
        Idle,
        Busy,
        Ready,
    }

    #[handlers]
    impl Worker {
        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push(format!("Idle sees {event:?}"));
            match event {
                Event::Start => goto!(Busy),
                Event::Work(_) => defer!(),
                _ => bubble!(),
            }
        }

        #[on(Busy)]
        fn on_busy(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push(format!("Busy sees {event:?}"));
            match event {
                Event::Done => goto!(Ready),
                Event::Work(_) => defer!(),
                _ => bubble!(),
            }
        }

        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push(format!("Ready sees {event:?}"));
            match event {
                Event::Work(_) => stay!(),
                Event::Start => goto!(Idle),
                _ => bubble!(),
            }
        }
    }
}

use worker::Worker;

fn main() {
    let mut machine = Machine::<Worker>::new(Vec::new());
    machine.send(Event::Work(1)).unwrap();
    machine.send(Event::Work(2)).unwrap();
    // `Idle` deferred both; nothing changed, so nothing was redelivered.
    assert_eq!(machine.ctx(), &["Idle sees Work(1)", "Idle sees Work(2)"]);
    machine.ctx_mut().clear();

    // The configuration changes: `Busy` sees the deferred events and defers them again.
    machine.send(Event::Start).unwrap();
    assert_eq!(
        machine.ctx(),
        &["Idle sees Start", "Busy sees Work(1)", "Busy sees Work(2)"]
    );
    machine.ctx_mut().clear();

    // `Ready` can handle them: they come back in their original order.
    machine.send(Event::Done).unwrap();
    assert_eq!(
        machine.ctx(),
        &["Busy sees Done", "Ready sees Work(1)", "Ready sees Work(2)"]
    );
}
```

`defer!()` in a state that has no way out never gets redelivered. The macro warns about that with [W0003](../errors/W0003.md).

### The defer queue is bounded

The defer queue holds `defer_cap` events (default 8). Set the size and the overflow behaviour on the machine:

- `defer_cap = N` sets the capacity.
- `defer_overflow = error` (default): `send` returns `Err(SendError::DeferOverflow(event))` and the event is not stored. If the event that overflowed is the one you sent, nothing ran: the machine is unchanged and not poisoned, so you can retry later. If it is a raised or redelivered event, transitions already ran in the same `send` and the rest of the step is lost, so the machine is **poisoned**. Size `defer_cap` for the events one step can defer, or use `drop_oldest`.
- `defer_overflow = drop_oldest`: the oldest deferred event is dropped to make room.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Work(u8),
    Done,
}

#[estado::machine(event = Event, defer_cap = 2)]
mod strict {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Strict {
        #[initial]
        Waiting,
        Served,
    }

    #[handlers]
    impl Strict {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Work(_) => defer!(),
                Event::Done => goto!(Served),
            }
        }

        #[on(Served)]
        fn on_served(_event: &Event) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

#[estado::machine(event = Event, defer_cap = 2, defer_overflow = drop_oldest)]
mod lossy {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lossy {
        #[initial]
        Waiting,
        Served,
    }

    #[handlers]
    impl Lossy {
        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Work(_) => defer!(),
                Event::Done => goto!(Served),
            }
        }

        #[on(Served)]
        fn on_served(event: &Event) -> Next<Self> {
            match event {
                Event::Work(_) => stay!(),
                Event::Done => goto!(Waiting),
            }
        }
    }
}

fn main() {
    // `error`: the third event is refused and given back.
    let mut strict = Machine::<strict::Strict>::new(());
    strict.send(Event::Work(1)).unwrap();
    strict.send(Event::Work(2)).unwrap();
    let error = strict.send(Event::Work(3)).unwrap_err();
    assert_eq!(error, SendError::DeferOverflow(Event::Work(3)));
    assert!(!strict.is_poisoned());

    // `drop_oldest`: all four are accepted, only the newest two survive.
    let mut lossy = Machine::<lossy::Lossy>::new(());
    for number in 1..=4 {
        lossy.send(Event::Work(number)).unwrap();
    }
    let report = lossy.send(Event::Done).unwrap().report;
    // One microstep for `Done`, one for each of the two redelivered events.
    assert_eq!(report.microsteps, 3);
}
```

## `cx.raise`

`cx.raise(event)` queues an event for the **same machine**. It is processed after the current microstep, before deferred events. Use it to split a reaction into steps, or to make one part of the machine tell another.

`raise` works in handlers, entries, exits, `#[always]` and `#[on_done]`. Raised events are FIFO. The queue holds `raise_cap` events (default 8). Overflowing it is a modelling bug: `send` returns `Err(SendError::RaiseOverflow)` and the machine is **poisoned**.

An entry that raises, with its consequences in order:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Loaded,
    Parsed,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod pipeline {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Pipeline {
        #[initial]
        Idle,
        Loading,
        Parsing,
        Done,
    }

    #[handlers]
    impl Pipeline {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Loading)
        }

        // Entering `Loading` raises two events, in order.
        #[entry(Loading)]
        fn enter_loading(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Loading".to_string());
            cx.raise(Event::Loaded);
            cx.raise(Event::Parsed);
        }

        #[on(Loading)]
        fn on_loading(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push(format!("Loading sees {event:?}"));
            match event {
                Event::Loaded => goto!(Parsing),
                _ => bubble!(),
            }
        }

        #[on(Parsing)]
        fn on_parsing(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            cx.ctx_mut().push(format!("Parsing sees {event:?}"));
            match event {
                Event::Parsed => goto!(Done),
                _ => bubble!(),
            }
        }

        #[on(Done)]
        fn on_done(_event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use pipeline::Pipeline;

fn main() {
    let mut machine = Machine::<Pipeline>::new(Vec::new());
    let report = machine.send(Event::Start).unwrap().report;

    // One `send`: the external event and both raised events.
    assert_eq!(
        machine.ctx(),
        &["enter Loading", "Loading sees Loaded", "Parsing sees Parsed"]
    );
    assert_eq!(*machine.state(), Pipeline::Done);
    assert_eq!(report.microsteps, 3);
}
```

A raised event that nobody handles is not an error. It is counted in `report.unhandled_internal`:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Echo,
}

#[estado::machine(event = Event)]
mod caller {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Caller {
        #[initial]
        Ready,
        Called,
    }

    #[handlers]
    impl Caller {
        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Go => {
                    cx.raise(Event::Echo);
                    goto!(Called)
                }
                _ => unhandled!(),
            }
        }

        #[on(Called)]
        fn on_called(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Ready),
                _ => unhandled!(),
            }
        }
    }
}

use caller::Caller;

fn main() {
    let mut machine = Machine::<Caller>::new(());
    let report = machine.send(Event::Go).unwrap().report;
    assert_eq!(report.unhandled_internal, 1);
}
```

### `raise_cap`

`raise_cap = N` changes the size of the raise queue. When the queue is full, the next `raise` is a fault:

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Burst,
    Ping,
}

#[estado::machine(event = Event, raise_cap = 1)]
mod chatty {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Chatty {
        #[initial]
        Quiet,
        Loud,
    }

    #[handlers]
    impl Chatty {
        #[entry(Loud)]
        fn enter_loud(cx: &mut Cx<Self>) {
            cx.raise(Event::Ping);
            cx.raise(Event::Ping); // does not fit
        }

        #[on(Quiet)]
        fn on_quiet(event: &Event) -> Next<Self> {
            match event {
                Event::Burst => goto!(Loud),
                _ => unhandled!(),
            }
        }

        #[on(Loud)]
        fn on_loud(_event: &Event) -> Next<Self> {
            goto!(Quiet)
        }
    }
}

use chatty::Chatty;

fn main() {
    let mut machine = Machine::<Chatty>::new(());
    let error = machine.send(Event::Burst).unwrap_err();
    assert_eq!(error, SendError::RaiseOverflow);
    assert!(machine.is_poisoned());
    assert!(matches!(machine.send(Event::Burst), Err(SendError::Poisoned)));
}
```

The macro looks for `cx.raise(..)` and `defer!()` inside the machine module, and builds the queues only when it finds one. A `raise` written in a helper function outside the module is invisible to it. Write `raise_cap = N` (or `defer_cap = N`) to force the queue on. See [Warnings and options](./22-warnings-and-options.md).

## Next

[Testing](./19-testing.md) shows how to drive a machine with a virtual clock.
