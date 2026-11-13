# Core

`estado-core` is the engine under the macro. It has no dependencies, is `no_std` without `alloc`, and holds everything a running machine needs: the `Spec` trait, `Machine`, `Cx`, effects, sinks and the static graph. The facade crate `estado` re-exports all of it, so `estado::Spec` and `estado_core::Spec` are the same item.

You can use it without `#[machine]`. The macro only writes a `Spec` implementation for you; everything it generates can be written by hand. Do this when you cannot use proc macros, when you want full control of the generated shape, or to understand what the macro does. Most users should use the macro; this page is for the rest.

Complete hand-written machines, each next to its macro version, are in the repository:

| Machine | Hand-written | With the macro | Shows |
| --- | --- | --- | --- |
| Media player | [`i_media_player.rs`](../../examples/i_media_player.rs) | [`i_media_player_macro.rs`](../../examples/i_media_player_macro.rs) | Three levels, `*Fields` and `*Mut` views, constructor edges |
| Connection | [`b_connection.rs`](../../examples/b_connection.rs) | [`b_connection_macro.rs`](../../examples/b_connection_macro.rs) | Timers, tasks, backoff, parallel keep-alive, defer |
| Game character | [`d_game_character.rs`](../../examples/d_game_character.rs) | [`d_game_character_macro.rs`](../../examples/d_game_character_macro.rs) | Three regions, history, `#[always]` |
| HTTP parser | [`a_http_parser.rs`](../../examples/a_http_parser.rs) | [`a_http_parser_macro.rs`](../../examples/a_http_parser_macro.rs) | Zero-cost dispatch on a byte stream |

Each hand-written example is split into several files, roughly one per kind of generated code (`model`, `actions`, `edges`, `handlers`, `dispatch`, and for most of them `views` and `graph`). The split differs per example. The media player keeps the `Spec` implementation in `dispatch.rs`. The connection and the game character keep it in `spec.rs`. The HTTP parser is the smallest: it has no views or graph file.

## Public items

| Item | What it is |
| --- | --- |
| `Spec` | The trait a machine implements. See [below](#the-spec-trait). |
| `Machine<M>` | A running instance. See [machine](machine.md). |
| `Cx<'step, M, S>` | The context handed to handlers. See [context](context.md). |
| `Next<M, Edges>`, `Edge<M>`, `NoEdge` | The result of a handler and its typed edges. |
| `Outcome`, `Slot<M>` | The result of a microstep, and the storage of the root state. |
| `Effect`, `TimerKey`, `InvokeKey`, `ChildPath` | What the machine asks of its runtime. See [effects](effects.md). |
| `Sink`, `Overflow`, `Discard`, `FnSink`, `InlineEffects`, `IntoEffects` | Where effects go. |
| `Driver`, `DriverSink`, `drive` | The runtime interface. |
| `Step`, `Report`, `IgnoreReason`, `SendError` | Results and errors of `send`. |
| `Runtime`, `EventQueues`, `NoRuntime`, `Queues`, `InlineQueue` | Auxiliary state: queues, epochs, history. |
| `UnhandledPolicy`, `DeferOverflowPolicy` | The policies a `Spec` declares. |
| `StateId`, `StatePath`, `MAX_ACTIVE_STATES` | Identifiers and the active configuration. |
| `Graph`, `StateInfo`, `StateKind`, `HistoryKind`, `EdgeInfo`, `EdgeKind`, `EdgeTarget` | The static graph. |

Items marked `#[doc(hidden)]` (`Cx::new`, `Cx::effect`, `Cx::runtime_mut`, `Cx::refresh_active`, `Cx::fault`, `Fault`, `Edge::apply`, `Machine::runtime`) exist for generated code. Hand-written `Spec` implementations in the examples use them; they are not stable API for application code.

## The Spec trait

```rust,ignore
pub trait Spec: Sized {
    type Event;
    type Effect;
    type Ctx;
    type Output;
    type Runtime: Runtime<Event = Self::Event>;
    type Effects: Sink<Self::Effect> + Default;

    const NAME: &'static str;
    const GRAPH: &'static Graph;
    const UNHANDLED: UnhandledPolicy = UnhandledPolicy::Ignore;
    const DEFER_OVERFLOW: DeferOverflowPolicy = DeferOverflowPolicy::Error;
    const MAX_MICROSTEPS: u16 = 64;
    const TRACKS_ACTIVE: bool = false;

    fn initial() -> Self;
    fn state_id(&self) -> StateId;
    fn path(&self) -> StatePath;
    fn enter_initial<S: Sink<Self::Effect>>(&mut self, cx: &mut Cx<'_, Self, S>);
    fn microstep<S: Sink<Self::Effect>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &Self::Event,
    ) -> Outcome;

    // Provided, with defaults:
    fn run_always<S: Sink<Self::Effect>>(slot: &mut Slot<Self>, cx: &mut Cx<'_, Self, S>) -> Outcome;
    fn is_done(&self) -> bool;
    fn into_output(self) -> Result<Self::Output, Self>;
    fn timer_event(&self, runtime: &Self::Runtime, key: TimerKey) -> Option<Self::Event>;
    fn accepts_invoke(&self, runtime: &Self::Runtime, key: InvokeKey) -> bool;
}
```

`Self` is the root state type. For a `#[machine]` it is the `#[root]` enum.

### Associated types

| Type | Meaning |
| --- | --- |
| `Event` | The events the machine reacts to. |
| `Effect` | The user effects (commands) it emits. `()` when none. |
| `Ctx` | The global context. `()` when none. |
| `Output` | The payload of the root `#[terminal]` state, or `()`. |
| `Runtime` | Auxiliary state: queues, epochs, history. Zero-sized when unused; use `NoRuntime<Event>`. |
| `Effects` | The buffer of `Machine::send`, usually `InlineEffects<Effect, N>`. Any `Sink + Default` works. |

### Constants

| Constant | Default | Meaning |
| --- | --- | --- |
| `NAME` | required | The machine name, used in panic messages. |
| `GRAPH` | required | The static [graph](#graph). |
| `UNHANDLED` | `Ignore` | The policy for events nobody handles. |
| `DEFER_OVERFLOW` | `Error` | What a full defer queue does. |
| `MAX_MICROSTEPS` | `64` | The [livelock budget](errors.md#the-livelock-budget). |
| `TRACKS_ACTIVE` | `false` | Whether `Machine` snapshots `path()` before each microstep so that `Cx::in_state` works. |

### Required methods

| Method | What it must do |
| --- | --- |
| `initial()` | Build the root value of the initial state, before any entry runs. |
| `state_id(&self)` | The id of the active leaf (or of the parallel state). |
| `path(&self)` | The active configuration as a `StatePath`: push the ids in pre-order. |
| `enter_initial(&mut self, cx)` | Run the entry handlers from the root down to the initial leaves, and start their timers. |
| `microstep(slot, cx, event)` | Decide what happens to one event and, for a transition, run it. |

`microstep` is the core. `slot` is always `Slot::Live` on entry and on return. Return the `Outcome`:

| `Outcome` | Meaning |
| --- | --- |
| `Stayed` | Handled, no transition. |
| `Transitioned { changed }` | A transition ran. `changed` is whether the active configuration differs. A `true` redelivers deferred events. |
| `Deferred` | Keep the event in the defer queue. |
| `Unhandled(policy)` | Nobody handled it. Pass `Self::UNHANDLED`, or the override of a state. `Machine` applies the policy. |

To change the state, take it out of the slot and put the new one back: `slot.take()` leaves the slot poisoned until `slot.put(new)`, so a panic in between poisons the machine. `Slot` has `Live(state)`, `Unstarted`, `Poisoned` and `Finished`; `live()` and `live_mut()` borrow the state.

### Provided methods

| Method | Default | Override to |
| --- | --- | --- |
| `run_always(slot, cx)` | `Outcome::Stayed` | Evaluate pending `#[always]` handlers. `Machine` calls it when the runtime's `take_always_pending()` returns `true`. |
| `is_done(&self)` | `false` | Report that the root is in a final state. |
| `into_output(self)` | `Err(self)` | Move the output out of a final state. |
| `timer_event(&self, runtime, key)` | `None` | Return the event of a timer, or `None` when the key is stale. Called by `Machine::fire`. |
| `accepts_invoke(&self, runtime, key)` | `false` | Return whether the key belongs to the active state instance. Called by `Machine::complete`. |

### Handlers by hand

A handler is just a function. It returns `Next<Self, E>`: `Stay`, `Bubble`, `Unhandled`, `Defer`, `Reenter`, or `Goto(edge)`. `E` is a type implementing `Edge<Self>`; `NoEdge` is the empty edge set of a handler that never transitions. The macro generates one edge enum per handler. Writing an `Edge` by hand means implementing `Edge::apply`, which runs the exit and entry sequence of a transition; the examples above show it. A machine without hierarchy can skip `Next` and return an `Outcome` from `microstep` directly.

## A flat machine by hand

```rust
use estado::{
    Cx, Graph, InlineEffects, Machine, NoRuntime, Outcome, Sink, Slot, Spec, StateId, StateInfo,
    StateKind, StatePath,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Open,
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Light(bool),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Door {
    Closed,
    Open,
}

static STATES: [StateInfo; 2] = [
    StateInfo::new(StateId::new(0), "Closed", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(1), "Open", None, StateKind::Atomic, 0),
];
static GRAPH: Graph = Graph {
    machine: "Door",
    states: &STATES,
    edges: &[],
};

impl Spec for Door {
    type Event = Event;
    type Effect = Command;
    type Ctx = ();
    type Output = ();
    type Runtime = NoRuntime<Event>;
    type Effects = InlineEffects<Command, 2>;

    const NAME: &'static str = "Door";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Door::Closed
    }

    fn state_id(&self) -> StateId {
        StateId::new(*self as u16)
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(Self::GRAPH);
        path.push(self.state_id());
        path
    }

    fn enter_initial<S: Sink<Command>>(&mut self, _cx: &mut Cx<'_, Self, S>) {}

    fn microstep<S: Sink<Command>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &Event,
    ) -> Outcome {
        let next = match (slot.take(), event) {
            (Door::Closed, Event::Open) => {
                cx.emit(Command::Light(true));
                Door::Open
            }
            (Door::Open, Event::Close) => {
                cx.emit(Command::Light(false));
                Door::Closed
            }
            (state, _) => {
                slot.put(state);
                return Outcome::Unhandled(Self::UNHANDLED);
            }
        };
        slot.put(next);
        Outcome::Transitioned { changed: true }
    }
}

fn main() {
    let mut machine = Machine::<Door>::new(());

    let step = machine.send(Event::Open).unwrap();
    assert_eq!(step.effects.commands().collect::<Vec<_>>(), [&Command::Light(true)]);
    assert_eq!(machine.path().to_string(), "Open");

    // Nobody handles `Open` while open: the default policy ignores it.
    let step = machine.send(Event::Open).unwrap();
    assert!(!step.report.handled);
}
```

## Queues by hand

A machine that raises internal events or defers events needs a `Runtime` with queues. `Queues<Event, RAISE, DEFER>` is a ready-made set: the raise queue, the defer queue and the requeue. Put it in your runtime struct and return it from `Runtime::queues`.

```rust
use estado::{
    Cx, Discard, Graph, Machine, Outcome, Queues, Runtime, Sink, Slot, Spec, StateId, StateInfo,
    StateKind, StatePath,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Start,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Job {
    Idle,
    Done,
}

pub struct JobRuntime {
    queues: Queues<Event, 4, 4>,
}

impl Runtime for JobRuntime {
    type Event = Event;
    type Queues = Queues<Event, 4, 4>;
    const NEW: Self = JobRuntime {
        queues: Queues::new(),
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }
}

static STATES: [StateInfo; 2] = [
    StateInfo::new(StateId::new(0), "Idle", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(1), "Done", None, StateKind::Atomic, 0),
];
static GRAPH: Graph = Graph {
    machine: "Job",
    states: &STATES,
    edges: &[],
};

impl Spec for Job {
    type Event = Event;
    type Effect = ();
    type Ctx = ();
    type Output = ();
    type Runtime = JobRuntime;
    type Effects = Discard;

    const NAME: &'static str = "Job";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Job::Idle
    }

    fn state_id(&self) -> StateId {
        StateId::new(*self as u16)
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(Self::GRAPH);
        path.push(self.state_id());
        path
    }

    fn enter_initial<S: Sink<()>>(&mut self, _cx: &mut Cx<'_, Self, S>) {}

    fn microstep<S: Sink<()>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &Event,
    ) -> Outcome {
        match (*slot.live_mut(), event) {
            (Job::Idle, Event::Start) => {
                // Processed after this microstep, in the same `send`.
                cx.raise(Event::Finish);
                Outcome::Stayed
            }
            (Job::Idle, Event::Finish) => {
                slot.put(Job::Done);
                Outcome::Transitioned { changed: true }
            }
            _ => Outcome::Unhandled(Self::UNHANDLED),
        }
    }
}

fn main() {
    let mut machine = Machine::<Job>::new(());
    let step = machine.send(Event::Start).unwrap();

    assert_eq!(*machine.state(), Job::Done);
    assert_eq!(step.report.microsteps, 2); // `Start`, then the raised `Finish`
}
```

## Graph

`Spec::GRAPH` is a `&'static Graph`. The macro generates it as a `const`; a hand-written machine builds it with `StateInfo::new` and the `with_*` methods. Both read the same way:

```rust,ignore
let graph = <Player as Spec>::GRAPH;
let playing = graph.find("Playing").unwrap();
assert!(graph.is_ancestor_or_self(Player::ACTIVE_ID, playing.id));
```

| Item | Fields and methods |
| --- | --- |
| `Graph` | `machine: &'static str`, `states: &'static [StateInfo]`, `edges: &'static [EdgeInfo]`; `state(id)`, `find(name)`, `is_ancestor_or_self(ancestor, state)` |
| `StateInfo` | `id`, `name`, `parent`, `kind`, `initial`, `history`, `timers`, `invokes`, `depth`; builders `new`, `with_initial`, `with_history`, `with_tasks` |
| `StateKind` | `Atomic`, `Compound`, `Parallel`, `Region`, `Final`, `Choice` |
| `HistoryKind` | `None`, `Shallow`, `DeepKeep` |
| `EdgeInfo` | `from: StateId`, `to: EdgeTarget`, `kind: EdgeKind`, `event_hint: Option<&'static str>` |
| `EdgeTarget` | `State(id)`, `Choice(id)`, `Dynamic` |
| `EdgeKind` | `Normal`, `External`, `History`, `Error`, `Timer`, `Always`, `Done` |

State ids are indexes into `Graph::states`, in declaration order. The root enum itself is not a state of the graph. A `#[terminal]` state is `StateKind::Final` in the generated graph. Only `StateKind::Choice`, `EdgeTarget::Choice`, `EdgeTarget::Dynamic` and `EdgeKind::Error` are reserved: the macro does not emit them yet.

The planned `estado-graph` crate will export graphs to Mermaid, DOT, SCXML and JSON. It is not implemented yet.
