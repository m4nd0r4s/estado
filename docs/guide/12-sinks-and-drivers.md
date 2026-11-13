# Sinks and drivers

## Choosing where effects go

`send` is the convenient form. It builds a fresh `InlineEffects` on every call. When you want the effects somewhere else, use the `_into` forms and pass a **sink**. A sink is anything that implements `Sink<Command>`:

| Sink | Use |
| --- | --- |
| `Vec<Effect<Command>>` | Collects into your own vector (needs `alloc`). |
| `InlineEffects<Command, N>` | A buffer of your own size. |
| `Discard` | Drops every effect. |
| `FnSink(closure)` | Calls a closure for each effect. |
| `DriverSink(&mut driver)` | Hands each effect to a `Driver`, as it is produced. |

Each of these methods has an `_into` form: `send_into(event, &mut sink)`, `start_into`, `fire_into` and `complete_into`. `send_with(event, closure)` is a shortcut for a closure sink. `send_batch(events, &mut sink)` sends many events and stops at the first error.

```rust
use estado::prelude::*;
use estado::{Effect, FnSink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Ping,
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Pong,
}

#[estado::machine(event = Event, effect = Command)]
mod echo {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Echo {
        #[initial]
        Ready,
        #[terminal]
        Stopped,
    }

    #[handlers]
    impl Echo {
        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Ping => {
                    cx.emit(Command::Pong);
                    stay!()
                }
                Event::Stop => goto!(Stopped),
            }
        }
    }
}

use echo::Echo;

fn main() {
    let mut machine = Machine::<Echo>::new(());

    // Into a vector of your own.
    let mut effects: Vec<Effect<Command>> = Vec::new();
    machine.send_into(Event::Ping, &mut effects).unwrap();
    assert_eq!(effects, [Effect::User(Command::Pong)]);

    // Into a closure.
    let mut count = 0;
    machine.send_into(Event::Ping, &mut FnSink(|_effect: Effect<Command>| count += 1)).unwrap();
    assert_eq!(count, 1);

    // Dropped.
    machine.send_into(Event::Ping, &mut Discard).unwrap();

    // Many events at once; the report adds the counters up.
    let report = machine.send_batch([Event::Ping, Event::Ping, Event::Ping], &mut Discard).unwrap();
    assert_eq!(report.microsteps, 3);
}
```

## Writing a sink

A sink is one method: `push(&mut self, effect: Effect<Command>) -> Result<(), Overflow>`. Return `Err(Overflow)` when you have no room, and `send` reports `SendError::EffectOverflow`.

A sink of your own can sort effects as they arrive. This one files user commands, timers and tasks into separate lists. It is also a good way to see all four `_into` methods side by side: `start_into`, `send_into`, `fire_into` and `complete_into`.

```rust
use core::time::Duration;
use estado::prelude::*;
use estado::{Effect, InvokeKey, Overflow, TimerKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Begin,
    Timeout,
    Done(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Greet,
    Work(u32),
}

#[estado::machine(event = Event, effect = Command)]
mod job {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Job {
        #[initial]
        Idle,
        #[after(Duration::from_secs(5) => Event::Timeout)]
        #[invoke(Command::Work(42))]
        Running,
        Finished { answer: u32 },
    }

    #[handlers]
    impl Job {
        #[entry(Idle)]
        fn enter_idle(cx: &mut Cx<Self>) {
            cx.emit(Command::Greet);
        }

        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Begin => goto!(Running),
                _ => unhandled!(),
            }
        }

        #[on(Running)]
        fn on_running(event: &Event) -> Next<Self> {
            match event {
                Event::Done(answer) => goto!(Finished { answer: *answer }),
                Event::Timeout => goto!(Idle),
                _ => unhandled!(),
            }
        }

        #[on(Finished)]
        fn on_finished(_finished: FinishedMut<'_>, _event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use job::Job;

/// Sorts effects into lists, as a runtime would.
#[derive(Default)]
struct Sorted {
    commands: Vec<Command>,
    timers: Vec<(TimerKey, Duration)>,
    tasks: Vec<(InvokeKey, Command)>,
    cancelled: usize,
}

impl Sink<Command> for Sorted {
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        match effect {
            Effect::User(command) => self.commands.push(command),
            Effect::StartTimer { key, after } => self.timers.push((key, after)),
            Effect::Invoke { key, task } => self.tasks.push((key, task)),
            Effect::CancelTimer(_) | Effect::Cancel(_) => self.cancelled += 1,
        }
        Ok(())
    }
}

fn main() {
    let mut machine = Machine::<Job>::new(());
    let mut sorted = Sorted::default();

    // `start_into` runs the initial entries.
    machine.start_into(&mut sorted).unwrap();
    assert_eq!(sorted.commands, [Command::Greet]);

    // `send_into` collects the timer and the task of `Running`.
    machine.send_into(Event::Begin, &mut sorted).unwrap();
    assert_eq!(sorted.timers[0].1, Duration::from_secs(5));
    assert_eq!(sorted.tasks[0].1, Command::Work(42));

    // `complete_into` delivers the task result. Leaving `Running` cancels both.
    let task_key = sorted.tasks[0].0;
    machine.complete_into(task_key, Event::Done(7), &mut sorted).unwrap();
    assert_eq!(*machine.state(), Job::Finished { answer: 7 });
    assert_eq!(sorted.cancelled, 2);

    // `fire_into` with the old timer key: stale, nothing happens.
    let timer_key = sorted.timers[0].0;
    let report = machine.fire_into(timer_key, &mut sorted).unwrap();
    assert!(report.ignored.is_some());
}
```

For one-off cases, `send_with(event, |effect| ..)` takes a closure, and `FnSink(closure)` does the same wherever a sink is expected:

```rust
use estado::prelude::*;
use estado::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Ping,
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Pong(u32),
}

#[estado::machine(event = Event, effect = Command, ctx = u32)]
mod counter {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Counter {
        #[initial]
        Counting,
        #[terminal]
        Stopped,
    }

    #[handlers]
    impl Counter {
        #[on(Counting)]
        fn on_counting(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Ping => {
                    *cx.ctx_mut() += 1;
                    let count = *cx.ctx();
                    cx.emit(Command::Pong(count));
                    stay!()
                }
                Event::Stop => goto!(Stopped),
            }
        }
    }
}

use counter::Counter;

fn main() {
    let mut machine = Machine::<Counter>::new(0);
    let mut seen = Vec::new();

    machine
        .send_with(Event::Ping, |effect| {
            if let Effect::User(Command::Pong(count)) = effect {
                seen.push(count);
            }
        })
        .unwrap();
    machine
        .send_with(Event::Ping, |effect| {
            if let Effect::User(Command::Pong(count)) = effect {
                seen.push(count);
            }
        })
        .unwrap();

    assert_eq!(seen, [1, 2]);
}
```

## `Driver`: a runtime in one trait

A **runtime** is whatever carries out the effects: a loop, a thread, a Tokio task, an embedded executor. The core ships the contract that every runtime implements. `Driver<Command>` has one method for each kind of effect:

```text
fn effect(&mut self, command: Command);
fn start_timer(&mut self, key: TimerKey, after: Duration);
fn cancel_timer(&mut self, key: TimerKey);
fn invoke(&mut self, key: InvokeKey, task: Command);
fn cancel(&mut self, key: InvokeKey);
```

`drive(effect, &mut driver)` calls the right method for one `Effect`. `DriverSink` does that for each effect as the machine emits it.

```rust
use core::time::Duration;
use estado::prelude::*;
use estado::{Driver, DriverSink, InvokeKey, TimerKey, drive};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Press,
    Quit,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Beep,
}

#[estado::machine(event = Event, effect = Command)]
mod buzzer {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Buzzer {
        #[initial]
        Quiet,
        #[terminal]
        Off,
    }

    #[handlers]
    impl Buzzer {
        #[on(Quiet)]
        fn on_quiet(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Press => {
                    cx.emit(Command::Beep);
                    stay!()
                }
                Event::Quit => goto!(Off),
            }
        }
    }
}

use buzzer::Buzzer;

/// A runtime that only records what it was asked to do.
#[derive(Default)]
struct Recorder {
    log: Vec<String>,
}

impl Driver<Command> for Recorder {
    fn effect(&mut self, command: Command) {
        self.log.push(format!("run {command:?}"));
    }
    fn start_timer(&mut self, _key: TimerKey, after: Duration) {
        self.log.push(format!("timer in {after:?}"));
    }
    fn cancel_timer(&mut self, _key: TimerKey) {}
    fn invoke(&mut self, _key: InvokeKey, _task: Command) {}
    fn cancel(&mut self, _key: InvokeKey) {}
}

fn main() {
    let mut machine = Machine::<Buzzer>::new(());
    let mut recorder = Recorder::default();

    // Each effect is driven as the machine emits it.
    machine.send_into(Event::Press, &mut DriverSink(&mut recorder)).unwrap();
    assert_eq!(recorder.log, ["run Beep"]);

    // Or drive the effects of a finished step yourself.
    let step = machine.send(Event::Press).unwrap();
    for effect in step.effects {
        drive(effect, &mut recorder);
    }
    assert_eq!(recorder.log, ["run Beep", "run Beep"]);
}
```

The Tokio and Embassy runtimes, which will implement `Driver` for you, are milestone M5 and do not exist yet. Until then you write a small driver like the one above, or use the test harness from [Testing](./19-testing.md).

## Next

[Regions](./13-regions.md) adds parallel states.
