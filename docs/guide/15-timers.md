# Timers and tasks

The machine itself knows nothing about time or asynchronous work. It describes them as **effects**, and your runtime carries them out. A state can ask for two things:

- `#[after(duration => event)]` schedules a **timer**. When it fires, the machine receives `event`.
- `#[invoke(task)]` starts a **task**. When the task finishes, you hand its result to the machine as an event.

Timers and tasks belong to a **state instance**: they start when the state is entered and are cancelled when it exits. An entry creates a new instance. A result that arrives for an instance that is gone is **stale**, and the machine discards it.

## `#[after]`

Put `#[after(duration => event)]` on a state variant or on a `#[state]` struct. On entry, the machine emits `Effect::StartTimer { key, after }`. On exit, it emits `Effect::CancelTimer(key)`.

To fire a timer, give its key back to the machine with `fire(key)`. If the key still names the active instance, the machine builds the event and runs it like `send`. Otherwise it returns `Ok` with `ignored = Some(IgnoreReason::Stale)`.

```rust
use core::time::Duration;
use estado::prelude::*;
use estado::{Effect, IgnoreReason, TimerKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
    Close,
    Timeout,
}

#[estado::machine(event = Event)]
mod door {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Door {
        #[initial]
        Closed,
        /// Closes by itself after 30 seconds.
        #[after(Duration::from_secs(30) => Event::Timeout)]
        Open,
    }

    #[handlers]
    impl Door {
        #[on(Closed)]
        fn on_closed(event: &Event) -> Next<Self> {
            match event {
                Event::Open => goto!(Open),
                _ => unhandled!(),
            }
        }

        #[on(Open)]
        fn on_open(event: &Event) -> Next<Self> {
            match event {
                Event::Timeout | Event::Close => goto!(Closed),
                _ => unhandled!(),
            }
        }
    }
}

use door::Door;

fn timer_key(effects: &[Effect<()>]) -> Option<TimerKey> {
    effects.iter().find_map(|effect| match effect {
        Effect::StartTimer { key, .. } => Some(*key),
        _ => None,
    })
}

fn main() {
    let mut machine = Machine::<Door>::new(());

    // Entering `Open` asks the runtime for a timer.
    let step = machine.send(Event::Open).unwrap();
    let effects: Vec<_> = step.effects.into_iter().collect();
    assert!(matches!(
        effects[..],
        [Effect::StartTimer { after, .. }] if after == Duration::from_secs(30)
    ));
    let key = timer_key(&effects).unwrap();

    // The runtime fires it: the machine builds `Event::Timeout` itself.
    machine.fire(key).unwrap();
    assert_eq!(*machine.state(), Door::Closed);

    // The same key again names an instance that is gone.
    let report = machine.fire(key).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
}
```

### Cancelling

Leaving a state cancels its timers. The cancellations come after every exit and before the entries of the next state:

```rust
use core::time::Duration;
use estado::prelude::*;
use estado::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Stop,
    Tick,
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod clock {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Clock {
        #[initial]
        Stopped,
        #[after(Duration::from_secs(1) => Event::Tick)]
        Running,
    }

    #[handlers]
    impl Clock {
        #[on(Stopped)]
        fn on_stopped(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Running),
                _ => unhandled!(),
            }
        }

        #[exit(Running)]
        fn exit_running(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("exit Running".to_string());
        }

        #[entry(Stopped)]
        fn enter_stopped(cx: &mut Cx<Self>) {
            cx.ctx_mut().push("enter Stopped".to_string());
        }

        #[on(Running)]
        fn on_running(event: &Event) -> Next<Self> {
            match event {
                Event::Tick => goto!(Running, external),
                Event::Stop => goto!(Stopped),
                _ => unhandled!(),
            }
        }
    }
}

use clock::Clock;

fn main() {
    let mut machine = Machine::<Clock>::new(Vec::new());
    machine.send(Event::Start).unwrap();
    machine.ctx_mut().clear();

    let step = machine.send(Event::Stop).unwrap();
    let effects: Vec<_> = step.effects.into_iter().collect();
    // The `exit` ran first, then the timer was cancelled.
    assert_eq!(machine.ctx(), &["exit Running", "enter Stopped"]);
    assert!(matches!(effects[..], [Effect::CancelTimer(_)]));
}
```

## Durations and events computed from data

Both the duration and the event can be closures over the state's data. The closure receives the data by **shared reference**:

| State declared as | The closure receives |
| --- | --- |
| Unit variant: `Idle` | `&()` |
| Newtype variant or `#[state]` struct: `Online(Online)` | `&Online`. For a compound or parallel state, the whole struct. |
| Struct-like or tuple variant: `Backoff { attempt: u8 }` | `&BackoffRef<'_>`, a generated view with a `&` per field |

The **duration** is computed when the state is entered. The **event** is built when the timer fires, from the data the state has **at that moment**. The machine never stores one event per timer.

```rust
use core::time::Duration;
use estado::prelude::*;
use estado::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Fail,
    Retry(u8),
    Deposit(u32),
    Expired(u32),
}

#[estado::machine(event = Event, ctx = Vec<String>)]
mod session {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Session {
        #[initial]
        Idle,
        // The wait doubles with each attempt: 100 ms, 200 ms, 400 ms ...
        #[after(|backoff| Duration::from_millis(100 << *backoff.attempt) => |backoff| Event::Retry(*backoff.attempt))]
        Backoff { attempt: u8 },
        // The event is built from the balance the state has when the timer fires.
        Account(Account),
    }

    #[state]
    #[after(Duration::from_secs(60) => |account: &Account| Event::Expired(account.balance))]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Account {
        pub balance: u32,
    }

    #[handlers]
    impl Session {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Fail => goto!(Backoff { attempt: 1 }),
                Event::Deposit(amount) => goto!(Account(Account { balance: *amount })),
                _ => unhandled!(),
            }
        }

        #[on(Backoff)]
        fn on_backoff(backoff: BackoffMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Retry(_) if *backoff.attempt < 3 => {
                    goto!(Backoff { attempt: *backoff.attempt + 1 })
                }
                Event::Retry(_) => goto!(Idle),
                _ => unhandled!(),
            }
        }

        #[on(Account)]
        fn on_account(account: &mut Account, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Deposit(amount) => {
                    account.balance += amount;
                    stay!()
                }
                Event::Expired(balance) => {
                    cx.ctx_mut().push(format!("expired with {balance}"));
                    goto!(Idle)
                }
                _ => unhandled!(),
            }
        }
    }
}

use session::Session;

fn first_timer(effects: &[Effect<()>]) -> (estado::TimerKey, Duration) {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::StartTimer { key, after } => Some((*key, *after)),
            _ => None,
        })
        .unwrap()
}

fn main() {
    // The duration depends on `attempt`.
    let mut machine = Machine::<Session>::new(Vec::new());
    let effects: Vec<_> = machine.send(Event::Fail).unwrap().effects.into_iter().collect();
    let (key, after) = first_timer(&effects);
    assert_eq!(after, Duration::from_millis(200));

    let effects: Vec<_> = machine.fire(key).unwrap().effects.into_iter().collect();
    let (_, after) = first_timer(&effects);
    assert_eq!(after, Duration::from_millis(400));

    // The event is built from the data at firing time.
    let mut machine = Machine::<Session>::new(Vec::new());
    let effects: Vec<_> = machine.send(Event::Deposit(10)).unwrap().effects.into_iter().collect();
    let (key, _) = first_timer(&effects);
    machine.send(Event::Deposit(5)).unwrap(); // the balance is 15 now
    machine.fire(key).unwrap();
    assert_eq!(machine.ctx(), &["expired with 15"]);
}
```

A state may have several `#[after]` attributes. Each has an index, which is part of the key:

```rust
use core::time::Duration;
use estado::prelude::*;
use estado::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Slow,
    Fast,
}

#[estado::machine(event = Event, ctx = Vec<&'static str>)]
mod pulse {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Pulse {
        #[initial]
        Idle,
        #[after(Duration::from_secs(1) => Event::Slow)]
        #[after(Duration::from_millis(250) => Event::Fast)]
        Beating,
    }

    #[handlers]
    impl Pulse {
        #[on(Idle)]
        fn on_idle(_event: &Event) -> Next<Self> {
            goto!(Beating)
        }

        #[on(Beating)]
        fn on_beating(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Slow => cx.ctx_mut().push("slow"),
                Event::Fast => cx.ctx_mut().push("fast"),
                Event::Start => return goto!(Idle),
            }
            stay!()
        }
    }
}

use pulse::Pulse;

fn main() {
    let mut machine = Machine::<Pulse>::new(Vec::new());
    let step = machine.send(Event::Start).unwrap();
    let timers: Vec<_> = step
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::StartTimer { key, after } => Some((*key, *after)),
            _ => None,
        })
        .collect();
    let indexes: Vec<_> = timers.iter().map(|(key, _)| key.index).collect();
    assert_eq!(indexes, [0, 1]);
    assert_eq!(timers[1].1, Duration::from_millis(250));

    // The runtime fires the shorter timer first.
    machine.fire(timers[1].0).unwrap();
    machine.fire(timers[0].0).unwrap();
    assert_eq!(machine.ctx(), &["fast", "slow"]);
}
```

## Stale timers and epochs

Every entry into a state with timers or tasks increments the **epoch** of that state. The key of a timer is `(state, epoch, index, child)`. After the state exits and enters again, the old key names an epoch that is over. Firing it is harmless, even though the state is active again:

```rust
use core::time::Duration;
use estado::prelude::*;
use estado::{Effect, IgnoreReason, TimerKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Reset,
    Beep,
}

#[estado::machine(event = Event, ctx = u32)]
mod alarm {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Alarm {
        #[initial]
        Armed,
        #[after(Duration::from_secs(5) => Event::Beep)]
        Ringing,
    }

    #[handlers]
    impl Alarm {
        #[on(Armed)]
        fn on_armed(event: &Event) -> Next<Self> {
            match event {
                Event::Reset => goto!(Ringing),
                _ => unhandled!(),
            }
        }

        #[on(Ringing)]
        fn on_ringing(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Reset => goto!(Armed),
                Event::Beep => {
                    *cx.ctx_mut() += 1;
                    stay!()
                }
            }
        }
    }
}

use alarm::Alarm;

fn key_of(effects: impl IntoIterator<Item = Effect<()>>) -> TimerKey {
    effects
        .into_iter()
        .find_map(|effect| match effect {
            Effect::StartTimer { key, .. } => Some(key),
            _ => None,
        })
        .unwrap()
}

fn main() {
    let mut machine = Machine::<Alarm>::new(0);

    let first = key_of(machine.send(Event::Reset).unwrap().effects); // epoch 1
    machine.send(Event::Reset).unwrap(); // back to `Armed`
    let second = key_of(machine.send(Event::Reset).unwrap().effects); // epoch 2
    assert_eq!((first.epoch, second.epoch), (1, 2));

    // `Ringing` is active, but `first` belongs to the earlier instance.
    let report = machine.fire(first).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(*machine.ctx(), 0);

    machine.fire(second).unwrap();
    assert_eq!(*machine.ctx(), 1);
}
```

This is why a runtime does not need to cancel in time. It may cancel a timer late, or never. The machine ignores stale firings either way.

## Next

[Tasks](./16-tasks.md) starts background work from a state.
