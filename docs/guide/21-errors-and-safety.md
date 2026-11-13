# Errors and safety

This page covers what can go wrong while a machine runs, and how `estado` keeps the machine safe when it does. Mistakes in the machine's *definition* are caught earlier, at compile time. They have their own error codes, listed in the [error index](../errors/README.md).

## `SendError`

`send`, `start`, `fire`, `complete` and their `_into` forms return `Result<_, SendError<Event>>`. The variants:

| Variant | Meaning | Poisons the machine? |
| --- | --- | --- |
| `Unhandled(event)` | No state handled the event and the policy is `error`. The event is given back. The configuration is unchanged. Data the handler changed before it answered stays changed. | no |
| `DeferOverflow(event)` | The defer queue is full under `defer_overflow = error`. The event that did not fit is given back. If it is the event you sent, nothing ran and the machine is unchanged. If it is a raised or redelivered event, transitions of the step already ran and the step is cut half-way. | **only for a raised or redelivered event** |
| `EffectOverflow` | The sink you passed has no room for an effect. The step stops half-way: the refused effect is lost and the rest of the step never runs. | **yes** |
| `RaiseOverflow` | The raise queue is full. A modelling bug. | **yes** |
| `Livelock { microsteps, last }` | A run took more than `max_microsteps` microsteps. | **yes** |
| `Poisoned` | An earlier panic or fatal error left the machine unusable. | already poisoned |

`SendError` implements `Display`, and `std::error::Error` when the `std` feature is on. The event type of `Unhandled` and `DeferOverflow` hands the event back so you can retry or log it.

## Unhandled events as errors

By default an unhandled event is not an error: `send` returns `Ok` and the report says `ignored = Some(IgnoreReason::Unhandled)`. With `unhandled = error`, `send` returns the event back instead. See [Unhandled events](./07-unhandled-events.md) for the per-state `#[unhandled]` override.

```rust
use estado::prelude::*;

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
    let mut machine = Machine::<Strict>::new(());
    let error = machine.send(Event::Noise).unwrap_err();
    assert_eq!(error, SendError::Unhandled(Event::Noise));
    assert_eq!(error.to_string(), "no state handled the event");

    // The configuration is unchanged and the machine is fine.
    assert!(!machine.is_poisoned());
    machine.send(Event::Go).unwrap();
}
```

## Poisoning

During the middle of a transition (after the old state left, before the new one is written) the machine holds no valid state. If a panic unwinds through that window, from a handler, an entry, an exit or a constructor, the machine stays **poisoned**. The same holds for a panic anywhere else while `send`, `start`, `fire` or `complete` processes an event. That includes the event expression of an `#[after]` timer, which `fire` evaluates when the timer fires. Every later `send` returns `Err(SendError::Poisoned)`. The same happens after `RaiseOverflow`, after `EffectOverflow`, after a `Livelock`, after a `DeferOverflow` of a raised or redelivered event, and after any error while the machine starts.

On a poisoned machine:

- `is_poisoned()` is `true`;
- `state()` and `path()` panic with a clear message;
- `try_state()` returns `None`.

```rust
use std::panic::{AssertUnwindSafe, catch_unwind};

use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Explode,
    Poke,
}

#[estado::machine(event = Event)]
mod fragile {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Fragile {
        #[initial]
        Whole,
        Broken,
    }

    #[handlers]
    impl Fragile {
        #[on(Whole)]
        fn on_whole(event: &Event) -> Next<Self> {
            match event {
                Event::Explode => goto!(Broken),
                _ => unhandled!(),
            }
        }

        #[entry(Broken)]
        fn enter_broken() {
            panic!("the entry blew up");
        }

        #[on(Broken)]
        fn on_broken(_event: &Event) -> Next<Self> {
            goto!(Whole)
        }
    }
}

use fragile::Fragile;

fn main() {
    let mut machine = Machine::<Fragile>::new(());
    machine.send(Event::Poke).unwrap();

    // The entry panics in the middle of the transition.
    let outcome = catch_unwind(AssertUnwindSafe(|| machine.send(Event::Explode)));
    assert!(outcome.is_err());

    assert!(machine.is_poisoned());
    assert!(machine.try_state().is_none());
    assert!(matches!(machine.send(Event::Poke), Err(SendError::Poisoned)));
}
```

Catching a panic and carrying on with a poisoned machine is almost never right. Treat poisoning as a signal to rebuild the machine from scratch.

`unhandled = panic` makes an unhandled event panic, and so poison the machine. Use it in tests, or when an unhandled event is a bug you want to find early.

## Livelock and `max_microsteps`

A `send` handles the external event and everything it triggers: `#[always]` evaluations, raised events and redelivered events. Each is a **microstep**. A machine that keeps triggering itself would never return, so the run is capped at `max_microsteps` (default 64). When the cap is exceeded, `send` returns `Err(SendError::Livelock { microsteps, last })`, where `last` is the active configuration when it stopped, and the machine is poisoned.

Raise the cap with `max_microsteps = N` when a legitimate run needs more steps.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
}

// An `#[always]` that goes back to its own state never settles.
#[estado::machine(event = Event, max_microsteps = 10)]
mod spinner {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Spinner {
        #[initial]
        Idle,
        Spinning,
    }

    #[handlers]
    impl Spinner {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Spinning)
        }

        #[always(Spinning)]
        fn spin() -> Next<Self> {
            goto!(Spinning, external)
        }
    }
}

use spinner::Spinner;

fn main() {
    let mut machine = Machine::<Spinner>::new(());
    let error = machine.send(Event::Start).unwrap_err();

    let SendError::Livelock { microsteps, last } = error else {
        panic!("expected a livelock");
    };
    assert_eq!(microsteps, 11);
    assert_eq!(last.to_string(), "Spinning");
    assert!(machine.is_poisoned());
}
```

The macro cannot see every loop. If your machine can loop by design, such as a polling machine, break the loop with an external event instead of an `#[always]` or a raise.

## Effect overflow

`send` collects effects in an `InlineEffects` buffer. With the `alloc` feature (on by default through `std`), it spills to the heap and never overflows. Without `alloc`, a buffer of `fx_inline` slots can fill up, and the effect that does not fit makes `send` return `Err(SendError::EffectOverflow)`. The step has already half run by then, so the machine is poisoned: size `fx_inline` for the largest burst of one step.

You can meet the same error with a sink of your own that reports `Overflow`. See [Sinks and drivers](./12-sinks-and-drivers.md).

```rust
use estado::prelude::*;
use estado::{Effect, Overflow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Burst,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Beep,
}

#[estado::machine(event = Event, effect = Command)]
mod beeper {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Beeper {
        #[initial]
        Ready,
        Spent,
    }

    #[handlers]
    impl Beeper {
        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, _event: &Event) -> Next<Self> {
            cx.emit(Command::Beep);
            cx.emit(Command::Beep);
            goto!(Spent)
        }

        #[on(Spent)]
        fn on_spent(_event: &Event) -> Next<Self> {
            goto!(Ready)
        }
    }
}

use beeper::Beeper;

/// A sink with room for a single effect.
struct OneSlot(Option<Effect<Command>>);

impl Sink<Command> for OneSlot {
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        if self.0.is_some() {
            return Err(Overflow);
        }
        self.0 = Some(effect);
        Ok(())
    }
}

fn main() {
    let mut machine = Machine::<Beeper>::new(());
    let mut sink = OneSlot(None);

    let error = machine.send_into(Event::Burst, &mut sink).unwrap_err();
    assert_eq!(error, SendError::EffectOverflow);
    assert_eq!(error.to_string(), "the effect buffer is full; the machine is poisoned");
    // The first effect did arrive; the machine is poisoned.
    assert_eq!(sink.0, Some(Effect::User(Command::Beep)));
    assert!(machine.is_poisoned());
}
```

## Safety in short

- A transition either completes or leaves the machine poisoned. There is no half-written state you can observe.
- A panic never leaves the machine usable by accident: later calls return `Poisoned`.
- A timer or task result for a state that is gone is ignored, not delivered to the wrong state. See [Timers](./15-timers.md) and [Tasks](./16-tasks.md).
- The same events in the same order give the same result. The core uses no clock, randomness or hash iteration.
- The core forbids `unsafe` code.

For the exhaustive list of compile-time diagnostics, see the [error index](../errors/README.md) and the [errors reference](../reference/errors.md).

## Next

[Warnings and options](./22-warnings-and-options.md) covers the lints the macro emits and every option of `#[estado::machine(..)]`.
