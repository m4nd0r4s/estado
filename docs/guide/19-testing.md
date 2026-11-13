# Testing

Because a machine only describes effects, you can test it without a clock, a network or an async runtime. The `estado-test` crate gives you a **harness** that plays the runtime: it keeps a virtual clock, records the timers and tasks the machine asks for, and fires them when you say so.

Add it as a dev-dependency:

```toml
[dev-dependencies]
estado-test = { git = "https://github.com/m4nd0r4s/estado" }
```

## The harness

`Harness::new(machine)` wraps a `Machine`. Only `send` and `start` start the machine when it is not started yet. `advance` and `complete_key` on an unstarted machine do nothing: there are no timers or tasks yet. The virtual clock begins at zero and moves only when you call `advance`.

| Method | What it does |
| --- | --- |
| `start()` | Runs the initial entries. |
| `send(event)` | Sends an event. |
| `advance(duration)` | Moves the clock forward and fires every timer that falls due, earliest first. |
| `complete_invoke(predicate, event)` | Completes the oldest pending task whose command matches `predicate`. |
| `complete_key(key, event)` | Completes a task by key, even a stale one. |
| `now()` | The virtual time. |
| `pending_timers()`, `pending_invokes()` | What the machine scheduled and has not resolved. |
| `assert_no_pending_timers()` | Fails when a timer is still pending. |
| `machine()`, `machine_mut()` | The wrapped machine. |
| `view()` | The effects and report of the last operation. |

`send`, `start`, `advance` and the `complete_*` methods return a `StepView` with the **user effects** of the operation in `effects` and its `report`. Timers and tasks do not appear there. They go to the pending sets.

`StepView` also has two chainable checks: `expect_effect(|command| ..)` and `expect_no_effect()`. The `assert_state!(harness, pattern)` macro checks the root state against a pattern. The `DurationExt` trait gives `u64` values `secs()` and `millis()`, so you write `13.secs()`. It is implemented for `u64` only.

Every method panics with a clear message when the machine returns a `SendError`, so a test reads like a script. To test an error, use `machine_mut()`.

## A first test

```rust
use estado::prelude::*;
use estado_test::{Harness, assert_state};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Press,
    Release,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    LightOn,
    LightOff,
}

#[estado::machine(event = Event, effect = Command)]
mod switch {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Switch {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Switch {
        #[entry(On)]
        fn enter_on(cx: &mut Cx<Self>) {
            cx.emit(Command::LightOn);
        }
        #[entry(Off)]
        fn enter_off(cx: &mut Cx<Self>) {
            cx.emit(Command::LightOff);
        }

        #[on(Off)]
        fn on_off(event: &Event) -> Next<Self> {
            match event {
                Event::Press => goto!(On),
                _ => unhandled!(),
            }
        }

        #[on(On)]
        fn on_on(event: &Event) -> Next<Self> {
            match event {
                Event::Release => goto!(Off),
                _ => unhandled!(),
            }
        }
    }
}

use switch::Switch;

fn main() {
    let mut harness = Harness::new(Machine::<Switch>::new(()));

    // Starting runs the entry of `Off`.
    harness.start().expect_effect(|command| *command == Command::LightOff);

    harness
        .send(Event::Press)
        .expect_effect(|command| *command == Command::LightOn);
    assert_state!(harness, Switch::On);

    // Chained checks on the last step.
    harness.send(Event::Release).expect_effect(|command| *command == Command::LightOff);
    assert_state!(harness, Switch::Off);
    assert_eq!(harness.view().report.transitions, 1);

    // An event the state ignores produces no effect.
    harness.send(Event::Release).expect_no_effect();
}
```

## Timers and the virtual clock

`advance(duration)` moves the clock and fires every timer whose deadline falls inside the interval. Timers fire in deadline order, and in scheduling order when two share a deadline. The effects of every fired timer are collected in the returned view.

The next machine retries a connection. The wait before each retry doubles. The test plays that out without waiting.

```rust
use core::time::Duration;
use estado::prelude::*;
use estado_test::{DurationExt, Harness, assert_state};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connect,
    Timeout,
    Retry,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Dial(u8),
}

#[estado::machine(event = Event, effect = Command)]
mod connection {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Connection {
        #[initial]
        Idle,
        /// Gives up on the attempt after 3 seconds.
        #[after(Duration::from_secs(3) => Event::Timeout)]
        Connecting { attempt: u8 },
        /// Waits 1 s, then 2 s, then 4 s before the next attempt.
        #[after(|backoff| Duration::from_secs(1 << (*backoff.attempt - 1)) => Event::Retry)]
        Backoff { attempt: u8 },
    }

    #[handlers]
    impl Connection {
        #[entry(Connecting)]
        fn enter_connecting(connecting: ConnectingMut<'_>, cx: &mut Cx<Self>) {
            cx.emit(Command::Dial(*connecting.attempt));
        }

        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Connect => goto!(Connecting { attempt: 1 }),
                _ => unhandled!(),
            }
        }

        #[on(Connecting)]
        fn on_connecting(connecting: ConnectingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Timeout => goto!(Backoff { attempt: *connecting.attempt }),
                _ => unhandled!(),
            }
        }

        #[on(Backoff)]
        fn on_backoff(backoff: BackoffMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Retry => goto!(Connecting { attempt: *backoff.attempt + 1 }),
                _ => unhandled!(),
            }
        }
    }
}

use connection::Connection;

fn main() {
    let mut harness = Harness::new(Machine::<Connection>::new(()));
    harness
        .send(Event::Connect)
        .expect_effect(|command| *command == Command::Dial(1));
    assert_eq!(harness.pending_timers().len(), 1);

    // Two seconds are not enough: the timer needs three.
    harness.advance(2.secs()).expect_no_effect();
    assert_state!(harness, Connection::Connecting { attempt: 1 });
    assert_eq!(harness.now(), Duration::from_secs(2));

    // One more second fires it: `Timeout` moves to `Backoff`.
    harness.advance(1.secs());
    assert_state!(harness, Connection::Backoff { attempt: 1 });

    // The backoff of the first attempt is one second, then the next dial.
    harness
        .advance(1.secs())
        .expect_effect(|command| *command == Command::Dial(2));
    assert_state!(harness, Connection::Connecting { attempt: 2 });

    // One big step fires a chain of timers, in order: the timeout at 7 s,
    // the two-second backoff, and the third dial at 9 s.
    harness
        .advance(5.secs())
        .expect_effect(|command| *command == Command::Dial(3));
    assert_state!(harness, Connection::Connecting { attempt: 3 });
    assert_eq!(harness.now(), Duration::from_secs(9));
}
```

When a state is left, its timers leave the pending set. `assert_no_pending_timers()` checks that nothing is waiting:

```rust
use core::time::Duration;
use estado::prelude::*;
use estado_test::{DurationExt, Harness};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Arm,
    Disarm,
    Boom,
}

#[estado::machine(event = Event)]
mod fuse {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Fuse {
        #[initial]
        Safe,
        #[after(Duration::from_secs(10) => Event::Boom)]
        Armed,
        #[terminal]
        Blown,
    }

    #[handlers]
    impl Fuse {
        #[on(Safe)]
        fn on_safe(event: &Event) -> Next<Self> {
            match event {
                Event::Arm => goto!(Armed),
                _ => unhandled!(),
            }
        }

        #[on(Armed)]
        fn on_armed(event: &Event) -> Next<Self> {
            match event {
                Event::Disarm => goto!(Safe),
                Event::Boom => goto!(Blown),
                _ => unhandled!(),
            }
        }
    }
}

use fuse::Fuse;

fn main() {
    let mut harness = Harness::new(Machine::<Fuse>::new(()));
    harness.send(Event::Arm);
    assert_eq!(harness.pending_timers().len(), 1);

    harness.advance(4.secs());
    harness.send(Event::Disarm);
    harness.assert_no_pending_timers();

    // Nothing fires any more, however long the clock runs.
    harness.advance(60.secs());
    assert!(!harness.machine().is_done());

    harness.send(Event::Arm);
    harness.advance(10.secs());
    assert!(harness.machine().is_done());
}
```

## Next

[Testing tasks and errors](./20-testing-tasks-and-errors.md) completes tasks with the harness and checks errors.
