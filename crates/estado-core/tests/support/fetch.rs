//! `Fetch`: a flat machine with a timer and an invoke, written by hand as
//! the macro would generate it, for epochs and stale results (AC-04).
//!
//! - `Connecting { attempt }` has `#[after(|s| 100ms * attempt => Timeout(s.attempt))]`.
//! - `Loading { id }` has `#[invoke(|s| Command::Fetch(s.id))]`.

use core::time::Duration;

use estado_core::{
    ChildPath, Cx, Effect, Graph, InlineEffects, InvokeKey, NoRuntime, Outcome, Runtime, Sink,
    Slot, Spec, StateId, StateInfo, StateKind, StatePath, TimerKey,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetch {
    Idle,
    Connecting { attempt: u8 },
    Loading { id: u32 },
    Ready(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connect,
    Timeout(u8),
    /// Stays in `Connecting` and changes its data.
    Bump,
    Load(u32),
    Loaded(u32),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Fetch(u32),
}

pub const IDLE: StateId = StateId::new(0);
pub const CONNECTING: StateId = StateId::new(1);
pub const LOADING: StateId = StateId::new(2);
pub const READY: StateId = StateId::new(3);

static STATES: [StateInfo; 4] = [
    StateInfo::new(IDLE, "Idle", None, StateKind::Atomic, 0),
    StateInfo::new(CONNECTING, "Connecting", None, StateKind::Atomic, 0).with_tasks(1, 0),
    StateInfo::new(LOADING, "Loading", None, StateKind::Atomic, 0).with_tasks(0, 1),
    StateInfo::new(READY, "Ready", None, StateKind::Atomic, 0),
];
static GRAPH: Graph = Graph {
    machine: "Fetch",
    states: &STATES,
    edges: &[],
};

/// Generated runtime: one epoch per state with timers or invokes.
pub struct FetchRuntime {
    queues: NoRuntime<Event>,
    pub connecting_epoch: u32,
    pub loading_epoch: u32,
}

impl Runtime for FetchRuntime {
    type Event = Event;
    type Queues = NoRuntime<Event>;
    const NEW: Self = Self {
        queues: <NoRuntime<Event> as Runtime>::NEW,
        connecting_epoch: 0,
        loading_epoch: 0,
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }
}

type FetchCx<'step, S> = Cx<'step, Fetch, S>;

pub fn timer_key(epoch: u32) -> TimerKey {
    TimerKey {
        state: CONNECTING,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

pub fn invoke_key(epoch: u32) -> InvokeKey {
    InvokeKey {
        state: LOADING,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

impl Fetch {
    fn id(&self) -> StateId {
        match self {
            Self::Idle => IDLE,
            Self::Connecting { .. } => CONNECTING,
            Self::Loading { .. } => LOADING,
            Self::Ready(_) => READY,
        }
    }
}

fn note<S: Sink<Command>>(cx: &mut FetchCx<'_, S>, line: String) {
    cx.ctx_mut().push(line);
}

/// Steps 1-2 of spec 5.3: exit, then cancel the instance's tasks.
fn exit<S: Sink<Command>>(state: &Fetch, cx: &mut FetchCx<'_, S>) {
    note(cx, format!("exit {}", GRAPH.state(state.id()).name));
    match state {
        Fetch::Connecting { .. } => {
            let epoch = cx.runtime_mut().connecting_epoch;
            cx.effect(Effect::CancelTimer(timer_key(epoch)));
        }
        Fetch::Loading { .. } => {
            let epoch = cx.runtime_mut().loading_epoch;
            cx.effect(Effect::Cancel(invoke_key(epoch)));
        }
        _ => {}
    }
}

/// Steps 7-9: entry, new epoch, then schedule from the state data.
fn entry<S: Sink<Command>>(state: &Fetch, cx: &mut FetchCx<'_, S>) {
    note(cx, format!("entry {}", GRAPH.state(state.id()).name));
    match state {
        Fetch::Connecting { attempt } => {
            let runtime = cx.runtime_mut();
            runtime.connecting_epoch = runtime.connecting_epoch.wrapping_add(1);
            let epoch = runtime.connecting_epoch;
            cx.effect(Effect::StartTimer {
                key: timer_key(epoch),
                after: Duration::from_millis(100) * u32::from(*attempt),
            });
        }
        Fetch::Loading { id } => {
            let runtime = cx.runtime_mut();
            runtime.loading_epoch = runtime.loading_epoch.wrapping_add(1);
            let epoch = runtime.loading_epoch;
            cx.effect(Effect::Invoke {
                key: invoke_key(epoch),
                task: Command::Fetch(*id),
            });
        }
        _ => {}
    }
}

fn transition<S: Sink<Command>>(
    slot: &mut Slot<Fetch>,
    cx: &mut FetchCx<'_, S>,
    target: Fetch,
) -> Outcome {
    exit(slot.live_mut(), cx);
    slot.put(target);
    entry(slot.live_mut(), cx);
    Outcome::Transitioned { changed: true }
}

impl Spec for Fetch {
    type Event = Event;
    type Effect = Command;
    type Ctx = Vec<String>;
    type Output = ();
    type Runtime = FetchRuntime;
    type Effects = InlineEffects<Command, 4>;

    const NAME: &'static str = "Fetch";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Self::Idle
    }

    fn state_id(&self) -> StateId {
        self.id()
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        path.push(self.id());
        path
    }

    fn enter_initial<S: Sink<Command>>(&mut self, cx: &mut FetchCx<'_, S>) {
        entry(self, cx);
    }

    fn microstep<S: Sink<Command>>(
        slot: &mut Slot<Self>,
        cx: &mut FetchCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        let state = slot.live_mut();
        let name = GRAPH.state(state.id()).name;
        cx.ctx_mut().push(format!("{name} handles {event:?}"));
        let target = match (state, event) {
            (Fetch::Idle, Event::Connect) => Fetch::Connecting { attempt: 1 },
            (Fetch::Connecting { attempt }, Event::Bump) => {
                *attempt += 10;
                return Outcome::Stayed;
            }
            (Fetch::Connecting { .. }, Event::Timeout(attempt)) if *attempt < 3 => {
                Fetch::Connecting {
                    attempt: attempt + 1,
                }
            }
            (Fetch::Connecting { .. }, Event::Timeout(_)) => Fetch::Idle,
            (Fetch::Idle | Fetch::Ready(_), Event::Load(id)) => Fetch::Loading { id: *id },
            (Fetch::Loading { .. }, Event::Loaded(value)) => Fetch::Ready(*value),
            (Fetch::Loading { .. } | Fetch::Connecting { .. }, Event::Cancel) => Fetch::Idle,
            _ => return Outcome::Unhandled(Self::UNHANDLED),
        };
        transition(slot, cx, target)
    }

    fn timer_event(&self, runtime: &FetchRuntime, key: TimerKey) -> Option<Event> {
        match self {
            Fetch::Connecting { attempt } if key == timer_key(runtime.connecting_epoch) => {
                Some(Event::Timeout(*attempt))
            }
            _ => None,
        }
    }

    fn accepts_invoke(&self, runtime: &FetchRuntime, key: InvokeKey) -> bool {
        matches!(self, Fetch::Loading { .. }) && key == invoke_key(runtime.loading_epoch)
    }
}
