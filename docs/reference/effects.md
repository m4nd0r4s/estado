# Effects

The machine never does I/O. When something has to happen outside (send a command, start a timer, run a task) it emits an `Effect` into a sink. A runtime reads the effects, does the work, and feeds the results back with `Machine::fire` and `Machine::complete`.

All the types on this page are exported by `estado` (and by `estado-core`).

## Effect

```rust,ignore
pub enum Effect<Command> {
    User(Command),
    StartTimer { key: TimerKey, after: Duration },
    CancelTimer(TimerKey),
    Invoke { key: InvokeKey, task: Command },
    Cancel(InvokeKey),
}

impl<Command> Effect<Command> {
    pub fn map<Other>(self, convert: impl FnOnce(Command) -> Other) -> Effect<Other>;
}
```

`Command` is the `effect` option of the machine. It implements `Debug`, `Clone`, `PartialEq` and `Eq` when `Command` does.

| Variant | Emitted when | What the runtime does |
| --- | --- | --- |
| `User(command)` | A handler calls `cx.emit(command)`. | Runs the command. |
| `StartTimer { key, after }` | A state with `#[after]` is entered. | Waits `after`, then calls `machine.fire(key)`. |
| `CancelTimer(key)` | The state that owns the timer exits. | Cancels the timer. Firing it late is harmless: it is stale. |
| `Invoke { key, task }` | A state with `#[invoke]` is entered. | Runs `task`, then calls `machine.complete(key, event)` with the result as an event. |
| `Cancel(key)` | The state that owns the task exits. | Cancels the task. Completing it late is harmless: it is stale. |

`map` converts the user payload of `User` and `Invoke` and keeps the keys. It exists so that effects of a child machine can be wrapped by its parent.

Order within one transition:

1. the effects of the `#[exit]` handlers;
2. `CancelTimer` and `Cancel` for the states that exited;
3. the effects of the `#[entry]` handlers;
4. `StartTimer` and `Invoke` for the states that entered, parents first.

Effects of one kind keep their emission order.

## Sink

A sink receives the effects of a step.

```rust,ignore
pub trait Sink<Command> {
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow>;
}

pub struct Overflow;
```

`push` returns `Err(Overflow)` when there is no room. The step then ends with `SendError::EffectOverflow` and the machine is poisoned (see [errors](errors.md#senderror)).

| Sink | Behavior |
| --- | --- |
| `InlineEffects<Command, N>` | The default buffer of `send`. See [below](#inlineeffects). |
| `Discard` | Drops every effect. Useful for machines with `effect = ()` and for tests. |
| `FnSink(closure)` | Calls `closure(effect)` for each effect. Never overflows. `Machine::send_with` wraps a closure this way. |
| `Vec<Effect<Command>>` | Appends. Needs the `alloc` feature. Never overflows. |
| `&mut S` where `S: Sink` | Forwards. Lets you pass `&mut sink` to a function that takes a sink by value. |
| `DriverSink(&mut driver)` | Drives each effect as it is emitted. See [Driver](#driver-and-drive). |

```rust
use estado::prelude::*;
use estado::{Discard, FnSink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Press,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Click,
}

#[estado::machine(event = Event, effect = Command, allow(W0002))]
mod button {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Button {
        #[initial]
        Up,
        Down,
    }

    #[handlers]
    impl Button {
        #[entry(Down)]
        fn entry_down(cx: &mut Cx<Self>) {
            cx.emit(Command::Click);
        }

        #[on(Up)]
        fn on_up(event: &Event) -> Next<Self> {
            match event {
                Event::Press => goto!(Down),
            }
        }
    }
}

use button::Button;

fn main() {
    // Into a `Vec`.
    let mut machine = Machine::<Button>::new(());
    let mut collected: Vec<Effect<Command>> = Vec::new();
    machine.send_into(Event::Press, &mut collected).unwrap();
    assert_eq!(collected, [Effect::User(Command::Click)]);

    // Into a closure.
    let mut machine = Machine::<Button>::new(());
    let mut clicks = 0;
    machine
        .send_into(Event::Press, &mut FnSink(|_effect| clicks += 1))
        .unwrap();
    assert_eq!(clicks, 1);

    // Dropped.
    let mut machine = Machine::<Button>::new(());
    machine.send_into(Event::Press, &mut Discard).unwrap();
}
```

## InlineEffects

`InlineEffects<Command, CAPACITY>` is the effect buffer of `Machine::send`: the type of `Step::effects` for a `#[machine]`, with `CAPACITY` set by the `fx_inline` option (default 4). It holds `Effect<Command>` values.

```rust,ignore
impl<Command, const CAPACITY: usize> InlineEffects<Command, CAPACITY> {
    pub const fn new() -> Self;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = &Effect<Command>>;
    pub fn commands(&self) -> impl Iterator<Item = &Command>;
}
```

- The first `CAPACITY` effects are stored inline, with no allocation.
- With the `alloc` feature, further effects spill to the heap. Without it, `push` returns `Err(Overflow)`.
- `commands()` yields the payload of the `User` effects only, in order.
- It implements `Default`, `Debug` (when `Command: Debug`), `Sink<Command>` and `IntoIterator<Item = Effect<Command>>` (the iterator is `IntoEffects`).

## Driver and drive

A `Driver` is the interface a runtime implements to execute effects.

```rust,ignore
pub trait Driver<Command> {
    fn effect(&mut self, command: Command);
    fn start_timer(&mut self, key: TimerKey, after: Duration);
    fn cancel_timer(&mut self, key: TimerKey);
    fn invoke(&mut self, key: InvokeKey, task: Command);
    fn cancel(&mut self, key: InvokeKey);
}

pub fn drive<Command>(effect: Effect<Command>, driver: &mut impl Driver<Command>);
pub struct DriverSink<'driver, D>(pub &'driver mut D);
```

`drive` calls the matching method for one effect. `DriverSink` turns a driver into a `Sink`, so each effect is driven as soon as it is emitted. The methods are synchronous; an async runtime spawns its work from them.

## Keys and epochs

```rust,ignore
pub struct TimerKey  { pub state: StateId, pub epoch: u32, pub index: u8, pub child: ChildPath }
pub struct InvokeKey { pub state: StateId, pub epoch: u32, pub index: u8, pub child: ChildPath }
```

Both are `Copy`, `Eq`, `Ord` and `Hash`, so they work as map keys and as idempotency keys for a task.

| Field | Meaning |
| --- | --- |
| `state` | The `StateId` of the state that owns the timer or task. |
| `epoch` | A counter of that state's visits. It increases by one each time the state is entered, starting at 1. |
| `index` | The position of the `#[after]` (or `#[invoke]`) among those of the state, starting at 0. |
| `child` | The child machine that owns the state. Always `ChildPath::ROOT` today. |

The epoch is what makes late results harmless. When a state exits and is entered again, the old timer and task keys no longer match the active instance. `fire` and `complete` check the key against the active configuration and return `Ok` with `report.ignored == Some(IgnoreReason::Stale)` when it does not match, without running anything.

A runtime therefore needs no bookkeeping beyond the keys: when it cancels late, or not at all, the machine ignores the result.

## Feeding results back

```rust,ignore
machine.fire(key)            // a timer fired
machine.complete(key, event) // a task finished, with its result as an event
```

Both also exist as `fire_into` and `complete_into` with a sink. They process the event like `send`: the same run-to-completion step, the same errors, effects in the sink. `fire` builds the timer's event when it fires, from the current data of the state.

```rust
use core::time::Duration;

use estado::prelude::*;
use estado::{Driver, DriverSink, IgnoreReason, InvokeKey, TimerKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Dial,
    Timeout,
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Dial(u8),
}

#[estado::machine(event = Event, effect = Command, allow(W0002))]
mod link {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Link {
        #[initial]
        Idle,
        #[after(Duration::from_secs(3) => Event::Timeout)]
        #[invoke(|connecting| Command::Dial(*connecting.attempt))]
        Connecting {
            attempt: u8,
        },
        Online,
    }

    #[handlers]
    impl Link {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Dial => goto!(Connecting { attempt: 1 }),
                _ => unhandled!(),
            }
        }

        #[on(Connecting)]
        fn on_connecting(event: &Event) -> Next<Self> {
            match event {
                Event::Timeout => goto!(Idle),
                Event::Connected => goto!(Online),
                _ => unhandled!(),
            }
        }
    }
}

use link::Link;

/// A runtime that only records what it was asked to do.
#[derive(Default)]
struct Recorder {
    commands: Vec<Command>,
    timers: Vec<(TimerKey, Duration)>,
    tasks: Vec<(InvokeKey, Command)>,
    cancelled: usize,
}

impl Driver<Command> for Recorder {
    fn effect(&mut self, command: Command) {
        self.commands.push(command);
    }
    fn start_timer(&mut self, key: TimerKey, after: Duration) {
        self.timers.push((key, after));
    }
    fn cancel_timer(&mut self, _key: TimerKey) {
        self.cancelled += 1;
    }
    fn invoke(&mut self, key: InvokeKey, task: Command) {
        self.tasks.push((key, task));
    }
    fn cancel(&mut self, _key: InvokeKey) {
        self.cancelled += 1;
    }
}

fn main() {
    let mut machine = Machine::<Link>::new(());
    let mut driver = Recorder::default();

    machine
        .send_into(Event::Dial, &mut DriverSink(&mut driver))
        .unwrap();
    let (timer, after) = driver.timers[0];
    let (task, command) = driver.tasks[0].clone();
    assert_eq!(after, Duration::from_secs(3));
    assert_eq!(command, Command::Dial(1));
    assert_eq!((timer.epoch, timer.index), (1, 0));

    // The task finishes: the machine leaves `Connecting` and cancels the timer.
    machine
        .complete_into(task, Event::Connected, &mut DriverSink(&mut driver))
        .unwrap();
    assert_eq!(*machine.state(), Link::Online);
    assert_eq!(driver.cancelled, 2); // the timer and the task

    // The cancel did not reach the clock in time: the timer fires anyway.
    let report = machine
        .fire_into(timer, &mut DriverSink(&mut driver))
        .unwrap();
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(*machine.state(), Link::Online);
}
```

## Child paths

`ChildPath` is the route from a machine to a child machine: up to `ChildPath::MAX_DEPTH` (4) segments. `ChildPath::ROOT` is the machine itself, `child(segment)` appends one (and returns `None` past the maximum), and `segments()` lists them. Child machines are not supported yet, so every key carries `ChildPath::ROOT`.
