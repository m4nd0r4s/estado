# Machine

`Machine<M>` is a running instance of a machine. `M` is the root type: the `#[root]` enum of a `#[machine]` module, or any type that implements [`Spec`](core.md). The machine owns the root state, the context and the runtime data (queues, epochs, history). It does no I/O: every effect leaves through a [sink](effects.md).

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Toggle,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    LampOn,
    LampOff,
}

#[estado::machine(event = Event, effect = Command, ctx = u32)]
mod lamp {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lamp {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Lamp {
        #[entry(Off)]
        fn entry_off(cx: &mut Cx<Self>) {
            cx.emit(Command::LampOff);
        }

        #[entry(On)]
        fn entry_on(cx: &mut Cx<Self>) {
            *cx.ctx_mut() += 1;
            cx.emit(Command::LampOn);
        }

        #[on(Off)]
        fn on_off(event: &Event) -> Next<Self> {
            match event {
                Event::Toggle => goto!(On),
            }
        }

        #[on(On)]
        fn on_on(event: &Event) -> Next<Self> {
            match event {
                Event::Toggle => goto!(Off),
            }
        }
    }
}

use lamp::Lamp;

fn main() {
    // The context is the only argument of `new`; nothing runs yet.
    let mut machine = Machine::<Lamp>::new(0);
    assert!(machine.try_state().is_none());

    // `send` starts the machine implicitly: the entry of `Off` runs first.
    let step = machine.send(Event::Toggle).unwrap();
    assert_eq!(
        step.effects.commands().collect::<Vec<_>>(),
        [&Command::LampOff, &Command::LampOn]
    );
    assert!(step.report.handled);
    assert_eq!(*machine.state(), Lamp::On);
    assert_eq!(*machine.ctx(), 1);
}
```

## Construction and start

```rust,ignore
impl<M: Spec> Machine<M> {
    pub const fn new(ctx: M::Ctx) -> Self;
    pub fn start(&mut self) -> Result<Step<M::Effects>, SendError<M::Event>>;
    pub fn start_into<S: Sink<M::Effect>>(&mut self, sink: &mut S)
        -> Result<Report, SendError<M::Event>>;
}
```

| Method | What it does |
| --- | --- |
| `new(ctx)` | Builds an unstarted machine around `ctx`. It is a `const fn`. Pass `()` when the machine has no `ctx` option. |
| `start()` | Runs the entries from the root down to the initial leaves, then drains `#[always]` evaluations and raised events. Returns the effects of those entries. |
| `start_into(sink)` | Same, writing the effects into `sink`. |

- `start` does nothing and returns an empty `Report` when the machine is already started or finished.
- `start` returns `Err(SendError::Poisoned)` on a poisoned machine.
- Calling `start` is optional. The first `send` starts the machine, and the start and the event share one [livelock budget](errors.md#the-livelock-budget).
- Any error during a start poisons the machine, because the entries that already ran leave no consistent configuration to resume from.

## Sending events

```rust,ignore
pub fn send(&mut self, event: M::Event)
    -> Result<Step<M::Effects>, SendError<M::Event>>;
pub fn send_into<S: Sink<M::Effect>>(&mut self, event: M::Event, sink: &mut S)
    -> Result<Report, SendError<M::Event>>;
pub fn send_with(&mut self, event: M::Event, on_effect: impl FnMut(Effect<M::Effect>))
    -> Result<Report, SendError<M::Event>>;
pub fn send_batch<S: Sink<M::Effect>>(
    &mut self,
    events: impl IntoIterator<Item = M::Event>,
    sink: &mut S,
) -> Result<Report, SendError<M::Event>>;
```

Each call runs one event to completion before it returns: the event itself, then every `#[always]` evaluation, raised event and requeued deferred event that it caused.

| Method | Effects go to |
| --- | --- |
| `send` | A fresh `M::Effects` buffer, returned in the [`Step`](#step-and-report). |
| `send_into` | The sink you pass. Nothing is allocated by the machine. |
| `send_with` | A closure called once per effect, in emission order. It never overflows. |
| `send_batch` | The sink you pass, for every event in order. Stops at the first error. |

`send_batch` merges the reports:

- `transitions`, `microsteps` and `unhandled_internal` are summed, each clamped to `u16::MAX`.
- `handled` and `ignored` describe the last event.
- With an empty iterator the report is `Report::default()`.

A machine that is done (see [`is_done`](#reading-the-state)) ignores events: `send` returns `Ok` with `report.ignored == Some(IgnoreReason::Unhandled)`. A poisoned machine returns `Err(SendError::Poisoned)`. Every error is explained in [errors](errors.md).

## Timer and invoke results

```rust,ignore
pub fn fire(&mut self, key: TimerKey) -> Result<Step<M::Effects>, SendError<M::Event>>;
pub fn fire_into<S: Sink<M::Effect>>(&mut self, key: TimerKey, sink: &mut S)
    -> Result<Report, SendError<M::Event>>;
pub fn complete(&mut self, key: InvokeKey, event: M::Event)
    -> Result<Step<M::Effects>, SendError<M::Event>>;
pub fn complete_into<S: Sink<M::Effect>>(&mut self, key: InvokeKey, event: M::Event, sink: &mut S)
    -> Result<Report, SendError<M::Event>>;
```

A runtime calls these when a timer fires or a task finishes. See [effects](effects.md#keys-and-epochs) for the keys.

| Method | Behavior |
| --- | --- |
| `fire(key)` | When `key` belongs to the active instance of its state, builds the timer's event from the current data of the state and processes it like `send`. Otherwise returns `Ok` with `report.ignored == Some(IgnoreReason::Stale)`. |
| `complete(key, event)` | When `key` belongs to the active instance, processes `event` like `send`. Otherwise returns `Ok` with `Stale` and drops `event`. |

Both return `Err(SendError::Poisoned)` on a poisoned machine, and `Stale` on a machine that is not started or has finished.

## Reading the state

```rust,ignore
pub fn state(&self) -> &M;
pub fn try_state(&self) -> Option<&M>;
pub fn path(&self) -> StatePath;
pub fn is_in(&self, state: StateId) -> bool;
pub fn ctx(&self) -> &M::Ctx;
pub fn ctx_mut(&mut self) -> &mut M::Ctx;
pub fn is_done(&self) -> bool;
pub fn take_output(&mut self) -> Option<M::Output>;
pub fn is_poisoned(&self) -> bool;
```

| Method | Returns |
| --- | --- |
| `state()` | The root state. Panics when the machine is not started, poisoned or finished. |
| `try_state()` | `Some(&root)` when the machine is live, `None` otherwise. |
| `path()` | The active configuration as a [`StatePath`](#statepath). Panics under the same conditions as `state()`. |
| `is_in(id)` | Whether the state `id` is active. `false` when the machine is not live. Use the id constants, for example `Lamp::ON`. |
| `ctx()` / `ctx_mut()` | The global context. They work in every machine state, including poisoned. |
| `is_done()` | `true` when the root is in a `#[terminal]` state, or after `take_output`. `false` for an unstarted or poisoned machine. |
| `take_output()` | Moves the payload out of a root `#[terminal]` state and finishes the machine. `None` when the machine is not done. |
| `is_poisoned()` | Whether a panic or a fatal error poisoned the machine. |

`take_output` leaves the machine **finished**: `is_done()` stays `true`, `state()` and `path()` panic, `try_state()` returns `None`, later `take_output` calls return `None`, and `send` returns `Ok` with `ignored = Unhandled`.

```rust
use estado::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Finish(u32),
}

#[estado::machine(event = Event)]
mod job {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Job {
        #[initial]
        Running,
        #[terminal]
        Done(u32),
    }

    #[handlers]
    impl Job {
        #[on(Running)]
        fn on_running(event: &Event) -> Next<Self> {
            match event {
                Event::Finish(total) => goto!(Done(*total)),
            }
        }
    }
}

use job::Job;

fn main() {
    let mut machine = Machine::<Job>::new(());
    assert_eq!(machine.take_output(), None); // not done yet

    machine.send(Event::Finish(9)).unwrap();
    assert!(machine.is_done());
    assert_eq!(machine.take_output(), Some(9));

    // Finished: still done, state gone, events ignored.
    assert!(machine.is_done());
    assert!(machine.try_state().is_none());
    let step = machine.send(Event::Finish(1)).unwrap();
    assert!(!step.report.handled);
}
```

## Step and Report

```rust,ignore
pub struct Step<Effects> {
    pub effects: Effects,
    pub report: Report,
}

pub struct Report {
    pub handled: bool,
    pub transitions: u16,
    pub microsteps: u16,
    pub unhandled_internal: u16,
    pub ignored: Option<IgnoreReason>,
}

pub enum IgnoreReason {
    Unhandled,
    Stale,
}
```

`Step::effects` is `M::Effects`. For a `#[machine]` it is [`InlineEffects<Effect, N>`](effects.md#inlineeffects) with `N` set by the `fx_inline` option (default 4). It implements `IntoIterator<Item = Effect<Command>>`, and has `iter()`, `commands()`, `len()` and `is_empty()`.

| `Report` field | Meaning |
| --- | --- |
| `handled` | The external event was handled: a stay, a transition or a defer. `false` when nobody handled it. `false` for `start`, for `fire`/`complete` with a stale key, and when the machine is done. |
| `transitions` | Transitions run, counting every microstep. |
| `microsteps` | Microsteps run, counting the external event, `#[always]` evaluations, raised events and requeued deferred events. This is the number the [livelock budget](errors.md#the-livelock-budget) limits. |
| `unhandled_internal` | Internal or requeued events that nobody handled. They never cause an error. |
| `ignored` | `Some(Unhandled)`: the external event was unhandled under the `ignore` policy, or the machine is done. `Some(Stale)`: a `fire` or `complete` with a stale key. |

## StatePath

`StatePath` is the active configuration, stored inline (no allocation) with at most `MAX_ACTIVE_STATES` (32) states, in pre-order: parents before children, regions in declaration order.

The limit is 32 active states at once, counting every level and every region. Nothing checks it at compile time today. A machine that activates more than 32 states at once panics in `StatePath::push` while `send` runs. This is a known issue: it is planned to become a compile error. Until then, keep the number of simultaneously active states (the depth of the tree plus the states of all regions) at 32 or below.

```rust,ignore
impl StatePath {
    pub fn states(&self) -> &[StateId];
    pub fn contains(&self, state: StateId) -> bool;
    pub fn leaf(&self) -> Option<StateId>;
}
```

It implements `Clone`, `Copy`, `PartialEq`, `Eq`, `Debug` and `Display`. Equality compares the states only.

`Display` joins levels with `/`, names regions with `region:`, and separates regions with `,`:

```text
Idle
Active/Paused
Connected/link:Open,keepalive:Pinging
```

`leaf()` is the last state of the path. With regions it is the last state of the last region, not the parallel state.

## Not public

`Machine::runtime()` is `#[doc(hidden)]`; it exists for generated code and tests. A machine has no `Clone` or `Debug` implementation; read the state with `state()` and clone that.
