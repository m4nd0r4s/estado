# Testing

The `estado-test` crate runs a machine against an in-memory runtime and a virtual clock. A test sends events, advances time, completes tasks and asserts on the state and the effects, with no real timers and no async runtime. Because the machine does no I/O, the same machine is used unchanged in production.

```toml
[dev-dependencies]
estado-test = { git = "https://github.com/m4nd0r4s/estado" }
```

## What it exports

| Item | Kind | Purpose |
| --- | --- | --- |
| [`Harness<M>`](#harness) | struct | A machine, a virtual clock and the pending timers and tasks. |
| [`StepView<'_, Command>`](#stepview) | struct | The effects and the report of the last operation, with assertions. |
| [`PendingTimer`, `PendingInvoke`](#pending-timers-and-tasks) | structs | What the machine asked for and has not been resolved yet. |
| [`DurationExt`](#durationext) | trait | `13.secs()` and `250.millis()`. |
| [`assert_state!`](#assert_state) | macro | Asserts that the root state matches a pattern. |

`Harness<M>` needs `M::Event: Debug` and `M::Effect: Debug`.

## Harness

```rust,ignore
impl<M: Spec> Harness<M> where M::Effect: Debug, M::Event: Debug {
    pub fn new(machine: Machine<M>) -> Self;
    pub fn start(&mut self) -> StepView<'_, M::Effect>;
    pub fn send(&mut self, event: M::Event) -> StepView<'_, M::Effect>;
    pub fn advance(&mut self, duration: Duration) -> StepView<'_, M::Effect>;
    pub fn complete_invoke(&mut self, predicate: impl Fn(&M::Effect) -> bool, event: M::Event)
        -> StepView<'_, M::Effect>;
    pub fn complete_key(&mut self, key: InvokeKey, event: M::Event) -> StepView<'_, M::Effect>;
    pub fn now(&self) -> Duration;
    pub fn pending_timers(&self) -> &[PendingTimer];
    pub fn pending_invokes(&self) -> &[PendingInvoke<M::Effect>];
    pub fn assert_no_pending_timers(&self);
    pub fn machine(&self) -> &Machine<M>;
    pub fn machine_mut(&mut self) -> &mut Machine<M>;
    pub fn view(&self) -> StepView<'_, M::Effect>;
}
```

| Method | What it does |
| --- | --- |
| `new(machine)` | Wraps a machine. The clock starts at zero. The machine starts on the first operation, as usual. |
| `start()` | Runs the initial entries. |
| `send(event)` | Sends an event. |
| `advance(duration)` | Moves the clock forward and fires every timer that comes due, in deadline order; timers with the same deadline fire in the order they were scheduled. A timer scheduled by a fired timer can fire in the same call when it is due. The returned view holds the effects of all fired timers, and a report with their summed `microsteps` and `transitions`. |
| `complete_invoke(predicate, event)` | Completes the oldest pending task whose task effect matches `predicate`, delivering `event` as its result. Panics when none matches. |
| `complete_key(key, event)` | Delivers `event` as the result of an invoke key, pending or not. Use it to test stale results. |
| `now()` | The virtual time. |
| `pending_timers()` | Timers scheduled and neither fired nor cancelled. |
| `pending_invokes()` | Tasks started and neither completed nor cancelled. |
| `assert_no_pending_timers()` | Panics when a timer is pending, listing it. |
| `machine()` | The machine, for `state()`, `path()`, `ctx()`, `is_done()`. |
| `machine_mut()` | The machine, mutably. Calls through it bypass the harness: effects go to the sink you pass, and timers do not enter the pending sets. |
| `view()` | The view of the last operation again. |

The harness plays the runtime:

- `Effect::User` goes to the list in the view.
- `StartTimer` and `Invoke` go to the pending sets. Starting a timer with a key that is already pending replaces it.
- `CancelTimer` and `Cancel` remove them from the pending sets.
- A fired timer is removed from the pending set before the machine sees it.

**Errors panic.** Every operation panics when the machine returns a `SendError`, with the message ``machine `Name` failed: <display> (<debug>)``. To test an error, call the machine through `machine_mut()`.

## StepView

```rust,ignore
pub struct StepView<'harness, Command> {
    pub effects: &'harness [Command],
    pub report: Report,
}
```

`effects` holds the **user** effects of the operation (`Effect::User` payloads), in order. Timers and tasks are in the pending sets. `report` is the [`Report`](machine.md#step-and-report) of the operation.

| Method | Asserts |
| --- | --- |
| `expect_effect(predicate)` | Some user effect matches. Panics otherwise, listing the effects. Returns `&Self` for chaining. |
| `expect_no_effect()` | No user effect was emitted. Returns `&Self`. |

## Pending timers and tasks

```rust,ignore
pub struct PendingTimer {
    pub key: TimerKey,
    pub deadline: Duration,  // virtual time at which it fires
    pub sequence: u64,       // scheduling order, breaks deadline ties
}

pub struct PendingInvoke<Command> {
    pub key: InvokeKey,
    pub task: Command,
}
```

Both implement `Debug`, `Clone`, `PartialEq` and `Eq`.

## DurationExt

```rust,ignore
pub trait DurationExt {
    fn secs(self) -> Duration;
    fn millis(self) -> Duration;
}
```

Implemented for `u64`. `2.secs() + 500.millis()` is `Duration::from_millis(2500)`.

## assert_state

`assert_state!(harness, pattern)` asserts that `harness.machine().state()` matches `pattern`. An `if` guard is allowed: `assert_state!(harness, Uploader::Backoff { attempt } if *attempt > 2)`. The pattern is matched against a reference to the root state. On failure it prints the active path and the pattern.

It panics when the machine has no readable state: not started, poisoned or finished.

```rust
use core::time::Duration;

use estado::prelude::*;
use estado_test::{DurationExt, Harness, assert_state};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Timeout,
    Retry,
    Uploaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Upload(u8),
    Notify,
}

#[estado::machine(event = Event, effect = Command, unhandled = error, allow(W0002))]
mod uploader {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Uploader {
        #[initial]
        Idle,
        #[after(Duration::from_secs(10) => Event::Timeout)]
        #[invoke(|uploading| Command::Upload(*uploading.attempt))]
        Uploading {
            attempt: u8,
        },
        #[after(|backoff| Duration::from_secs(1 << *backoff.attempt) => Event::Retry)]
        Backoff {
            attempt: u8,
        },
        Done,
    }

    #[handlers]
    impl Uploader {
        #[entry(Done)]
        fn entry_done(cx: &mut Cx<Self>) {
            cx.emit(Command::Notify);
        }

        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Uploading { attempt: 1 }),
                _ => unhandled!(),
            }
        }

        #[on(Uploading)]
        fn on_uploading(uploading: UploadingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Timeout => goto!(Backoff {
                    attempt: *uploading.attempt
                }),
                Event::Uploaded => goto!(Done),
                _ => unhandled!(),
            }
        }

        #[on(Backoff)]
        fn on_backoff(backoff: BackoffMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Retry => goto!(Uploading {
                    attempt: *backoff.attempt + 1
                }),
                _ => unhandled!(),
            }
        }
    }
}

use uploader::Uploader;

fn main() {
    let mut harness = Harness::new(Machine::<Uploader>::new(()));

    // Timers and tasks are not user effects.
    harness.send(Event::Start).expect_no_effect();
    assert_state!(harness, Uploader::Uploading { attempt: 1 });
    assert_eq!(harness.pending_invokes().len(), 1);

    // The upload times out after ten virtual seconds; the backoff lasts 2^1 s.
    harness.advance(10.secs());
    assert_state!(harness, Uploader::Backoff { attempt: 1 });
    assert_eq!(harness.now(), 10.secs());
    assert_eq!(harness.pending_timers()[0].deadline, 12.secs());

    // Advance again to fire the backoff timer.
    harness.advance(2.secs());
    assert_state!(harness, Uploader::Uploading { attempt: 2 });

    // Complete the task of the second attempt.
    harness
        .complete_invoke(|task| matches!(task, Command::Upload(2)), Event::Uploaded)
        .expect_effect(|command| matches!(command, Command::Notify));
    assert_state!(harness, Uploader::Done);
    harness.assert_no_pending_timers();

    // Errors panic in the harness; go through the machine to check them.
    let result = harness.machine_mut().send(Event::Start);
    assert_eq!(result.unwrap_err(), SendError::Unhandled(Event::Start));
}
```
