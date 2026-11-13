# Errors

This page covers the errors a running machine returns (`SendError`), when a machine becomes poisoned, the livelock budget, and the compile-time codes the macro reports.

## SendError

```rust,ignore
pub enum SendError<Event> {
    Poisoned,
    Unhandled(Event),
    DeferOverflow(Event),
    RaiseOverflow,
    EffectOverflow,
    Livelock { microsteps: u16, last: StatePath },
}
```

It is returned by `start`, `send`, `send_into`, `send_with`, `send_batch`, `fire` and `complete` (and their `_into` forms). It implements `Debug`, `Clone`, `PartialEq`, `Eq` and `Display`, and, with the `std` feature, `std::error::Error` (when `Event: Debug`).

| Variant | When it happens | Poisons | State after |
| --- | --- | --- | --- |
| `Poisoned` | Any call on a machine that is already poisoned. | n/a | Unchanged; the machine stays poisoned. |
| `Unhandled(event)` | An external event nobody handled, under the `error` policy (the machine's `unhandled = error`, or an `#[unhandled(error)]` on the state). The event is given back. | No | Unchanged. |
| `DeferOverflow(event)` | `defer!()` with a full defer queue under `defer_overflow = error`. The event is given back. | No | Unchanged by the deferred event. Anything that ran earlier in the step stays applied. |
| `RaiseOverflow` | `cx.raise(..)` with a full internal queue (`raise_cap`). | Yes | Lost. |
| `EffectOverflow` | The sink refused an effect (`Overflow`). With the default `InlineEffects` this needs a build without `alloc`: with `alloc`, extra effects spill to the heap. | Only during a start | Kept. See below. |
| `Livelock { microsteps, last }` | A step ran more than `max_microsteps` microsteps. | Yes | Lost. |

The `Display` texts are:

```text
Poisoned        the machine is poisoned by an earlier panic or fatal error
Unhandled       no state handled the event
DeferOverflow   the defer queue is full
RaiseOverflow   the internal event queue overflowed; the machine is poisoned
EffectOverflow  the effect buffer is full
Livelock        livelock: 5 microsteps without settling, last in Ping; the machine is poisoned
```

Details:

- `Unhandled` is only returned for an **external** event. An internal event (raised, or redelivered after a deferral) that nobody handles is counted in `report.unhandled_internal` and is never an error.
- With the `panic` policy an unhandled event panics instead, and the panic poisons the machine.
- `EffectOverflow` outside a start is **not** treated as fatal. The microstep that overflowed has already run: its transition is applied and is not rolled back, and the effect the sink refused is lost. The rest of the step (raised events, `#[always]`, redelivery) is cut short. Treat it as a bug in the sink or in the `fx_inline` size. During `start`, or the implicit start of the first `send`, every error poisons.
- `last` of `Livelock` is the active configuration when the budget ran out. Its `Display` form names the states, for example `Ping`.

## Poisoning

A poisoned machine has lost its state. It has no way back: build a new `Machine`.

A machine becomes poisoned when:

1. Any code that runs while `send`, `start`, `fire` or `complete` processes an event **panics**: a handler, an entry, an exit, a constructor, a closure of a timer or task that runs inside the step, or the `panic` unhandled policy. The panic propagates to the caller as usual, and the machine is poisoned when it unwinds. With `panic = "abort"` the process ends instead.

   One known exception: the event closure of an `#[after]` timer runs inside `fire`, before the event is sent and before the guard that poisons. A panic there propagates but does not poison the machine. This is a known issue.
2. A step fails with `RaiseOverflow` or `Livelock`.
3. A `start` fails with any error, including an effect overflow in an entry.

A poisoned machine:

| Call | Result |
| --- | --- |
| `send`, `start`, `fire`, `complete` (any form) | `Err(SendError::Poisoned)` |
| `is_poisoned()` | `true` |
| `state()`, `path()` | panic with "is poisoned" |
| `try_state()` | `None` |
| `is_done()` | `false` |
| `is_in(id)` | `false` |
| `ctx()`, `ctx_mut()` | work; the context is not lost |

```rust
use std::panic::{AssertUnwindSafe, catch_unwind};

use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Work,
    Crash,
}

#[estado::machine(event = Event, ctx = u32, allow(W0002))]
mod worker {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Worker {
        #[initial]
        Ready,
    }

    #[handlers]
    impl Worker {
        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Work => {
                    *cx.ctx_mut() += 1;
                    stay!()
                }
                Event::Crash => panic!("a handler failed"),
            }
        }
    }
}

use worker::Worker;

fn main() {
    let mut machine = Machine::<Worker>::new(0);
    machine.send(Event::Work).unwrap();

    let outcome = catch_unwind(AssertUnwindSafe(|| machine.send(Event::Crash)));
    assert!(outcome.is_err());

    assert!(machine.is_poisoned());
    assert!(machine.try_state().is_none());
    assert_eq!(machine.send(Event::Work).unwrap_err(), SendError::Poisoned);
    assert_eq!(*machine.ctx(), 1); // the context survives
}
```

## The livelock budget

One `send` can run many microsteps: the external event, then every `#[always]` evaluation, raised event, completion and redelivered deferred event that follows. The budget stops a cycle that never settles.

- The budget is the `max_microsteps` option (default 64, `Spec::MAX_MICROSTEPS`).
- Every microstep counts: the external event, each `#[always]` and `#[on_done]` evaluation, each raised event, each requeued event.
- The check runs before each further microstep. When `microsteps` is already greater than the budget, `send` returns `Err(SendError::Livelock { microsteps, last })` and the machine is poisoned. `microsteps` is the budget plus one.
- The implicit start of a `send` on an unstarted machine shares the budget with the event. An explicit `start()` has its own.
- The effects that the aborted step already pushed to the sink stay there.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Spin,
}

#[estado::machine(event = Event, max_microsteps = 4, allow(W0001))]
mod spinner {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Spinner {
        #[initial]
        Idle,
        Ping,
        Pong,
    }

    #[handlers]
    impl Spinner {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Spin => goto!(Ping),
            }
        }

        // Each evaluation goes to the other state, which schedules the next one.
        #[always(Ping)]
        fn ping() -> Next<Self> {
            goto!(Pong)
        }

        #[always(Pong)]
        fn pong() -> Next<Self> {
            goto!(Ping)
        }
    }
}

use spinner::Spinner;

fn main() {
    let mut machine = Machine::<Spinner>::new(());
    let error = machine.send(Event::Spin).unwrap_err();
    match error {
        SendError::Livelock { microsteps, last } => {
            assert_eq!(microsteps, 5);
            assert_eq!(last.to_string(), "Ping");
        }
        other => panic!("expected a livelock, got {other:?}"),
    }
    assert!(machine.is_poisoned());
}
```

## Compile-time codes

Every diagnostic reads `estado::<code>: <message>`, with a `help:` line when there is a fix to suggest. Each code has a page with an example that triggers it and the fix.

### Errors

| Code | Condition |
| --- | --- |
| [E0001](../errors/E0001.md) | `#[machine]` not on an inline module with exactly one `#[root]` enum, or a marker outside a machine |
| [E0002](../errors/E0002.md) | `event` not declared |
| [E0003](../errors/E0003.md) | `goto!` target is not a state |
| [E0004](../errors/E0004.md) | `#[on(S)]` (or another role) with an unknown state |
| [E0005](../errors/E0005.md) | Duplicate handler for a role, or two roles on one function |
| [E0006](../errors/E0006.md) | Handler parameter not recognised, out of order or repeated |
| [E0010](../errors/E0010.md) | Level without exactly one `#[initial]` |
| [E0011](../errors/E0011.md) | Nesting deeper than 16 levels, or a state that contains itself |
| [E0012](../errors/E0012.md) | Duplicate state name, or a name equal to its own id constant |
| [E0013](../errors/E0013.md) | State mixes `#[substates]` and `#[region]` |
| [E0014](../errors/E0014.md) | Parallel state with a single region |
| [E0015](../errors/E0015.md) | Substates or region type is not a `#[state]` enum |
| [E0016](../errors/E0016.md) | Invalid `#[machine(..)]` option, or unknown code in `allow(..)` |
| [E0017](../errors/E0017.md) | Malformed state attribute, or a marker on the wrong item |
| [E0020](../errors/E0020.md) | `goto!` or `#[initial]` without the data a state needs |
| [E0021](../errors/E0021.md) | Transition between sibling regions |
| [E0022](../errors/E0022.md) | A `#[terminal]` state transitions out |
| [E0040](../errors/E0040.md) | Transition macro outside `#[handlers]` |
| [E0042](../errors/E0042.md) | Transition in an `#[entry]` or `#[exit]` |
| [E0043](../errors/E0043.md) | Malformed `goto!` |
| [E0044](../errors/E0044.md) | `#[on_done]` on a state that never completes |
| [E0045](../errors/E0045.md) | `reenter!`, `defer!`, `bubble!` or `unhandled!` in `#[always]` or `#[on_done]` |
| [E0046](../errors/E0046.md) | Constructor uses what the handler borrows |
| [E0050](../errors/E0050.md) | Regions in typestate mode |
| [E0060](../errors/E0060.md) | `async` handler |
| [E0061](../errors/E0061.md) | `Result` handler without `#[on_error]` |
| [E0090](../errors/E0090.md) | Feature not supported yet |

### Warnings

| Code | Condition |
| --- | --- |
| [W0001](../errors/W0001.md) | State unreachable from the initial state |
| [W0002](../errors/W0002.md) | State with no outgoing edges that is not `#[terminal]` |
| [W0003](../errors/W0003.md) | `defer!()` in a state with no outgoing edges |
| [W0004](../errors/W0004.md) | Variant larger than `size_warn` bytes |
| [W0005](../errors/W0005.md) | `#[after]` event that no handler handles (best effort) |

`W0010` (a typestate `match` arm not associated with an event variant) is reserved for the typestate mode and is not emitted yet.

Warnings are reported through rustc's `deprecated` lint, because stable proc macros cannot emit warnings. Two consequences:

- `#![deny(warnings)]` and `-D deprecated` turn them into errors, and `#![allow(deprecated)]` hides every `W` code.
- Prefer `#[machine(allow(W0002))]`: it generates nothing, whatever the lint level. An unknown code in `allow(..)` is [`E0016`](../errors/E0016.md).

When a machine has errors, the macro still emits a best-effort module next to the errors, so that rustc does not add a flood of follow-up errors. It skips the graph warnings W0001, W0002, W0003 and W0005, because the handler list is incomplete.
