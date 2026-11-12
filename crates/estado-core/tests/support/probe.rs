//! `Probe`: a flat machine written by hand against `Spec`, exercising the
//! send algorithm (queues, policies, poisoning). Every action is logged in
//! the context so tests can assert the exact order.
//!
//! States: `A` (initial, defers `Hold`), `B` (`#[unhandled(error)]`),
//! `C` (entry raises `Note(9)`, `#[always]` goes back to `A`).

use estado_core::{
    Cx, Edge, Graph, InlineEffects, Next, Outcome, Queues, Runtime, Sink, Slot, Spec, StateId,
    StateInfo, StateKind, StatePath, UnhandledPolicy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    A,
    B,
    C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go(Probe),
    RaiseTwice,
    Note(u8),
    Hold,
    Panic,
    PanicOnExit,
    Spin,
    Unknown,
    RaiseUnknown,
}

#[derive(Debug, Default)]
pub struct Log {
    pub lines: Vec<String>,
    pub panic_on_exit: bool,
}

pub struct ProbeRuntime {
    pub queues: Queues<Event, 2, 2>,
    always_pending: bool,
}

impl Runtime for ProbeRuntime {
    type Event = Event;
    type Queues = Queues<Event, 2, 2>;
    const NEW: Self = Self {
        queues: Queues::new(),
        always_pending: false,
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }

    fn take_always_pending(&mut self) -> bool {
        core::mem::take(&mut self.always_pending)
    }
}

static STATES: [StateInfo; 3] = [
    StateInfo::new(StateId::new(0), "A", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(1), "B", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(2), "C", None, StateKind::Atomic, 0),
];
static GRAPH: Graph = Graph {
    machine: "Probe",
    states: &STATES,
    edges: &[],
};

impl Probe {
    pub const A_ID: StateId = StateId::new(0);
    pub const B_ID: StateId = StateId::new(1);
    pub const C_ID: StateId = StateId::new(2);

    fn name(self) -> &'static str {
        GRAPH.state(self.id()).name
    }

    fn id(self) -> StateId {
        match self {
            Self::A => Self::A_ID,
            Self::B => Self::B_ID,
            Self::C => Self::C_ID,
        }
    }
}

type ProbeCx<'step, S> = Cx<'step, Probe, S>;

fn note<S: Sink<()>>(cx: &mut ProbeCx<'_, S>, line: String) {
    cx.ctx_mut().lines.push(line);
}

/// The edge set of the hand-written handlers: one simple-form `goto!` per
/// destination.
pub struct GotoEdge(Probe);

impl Edge<Probe> for GotoEdge {
    const TARGETS: &'static [StateId] = &[Probe::A_ID, Probe::B_ID, Probe::C_ID];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Probe>, cx: &mut ProbeCx<'_, S>) -> bool {
        let source = *slot.live_mut();
        exit(source, cx);
        slot.put(self.0);
        entry(self.0, cx);
        source != self.0
    }
}

fn exit<S: Sink<()>>(state: Probe, cx: &mut ProbeCx<'_, S>) {
    if cx.ctx().panic_on_exit {
        panic!("exit of {} panicked", state.name());
    }
    note(cx, format!("exit {}", state.name()));
}

fn entry<S: Sink<()>>(state: Probe, cx: &mut ProbeCx<'_, S>) {
    note(cx, format!("entry {}", state.name()));
    if state == Probe::C {
        cx.raise(Event::Note(9));
        cx.runtime_mut().always_pending = true;
    }
}

fn handle_common<S: Sink<()>>(cx: &mut ProbeCx<'_, S>, event: &Event) -> Next<Probe, GotoEdge> {
    match event {
        Event::Go(target) => Next::Goto(GotoEdge(*target)),
        Event::RaiseTwice => {
            cx.raise(Event::Note(1));
            cx.raise(Event::Note(2));
            Next::Stay
        }
        Event::Note(number) => {
            note(cx, format!("note {number}"));
            Next::Stay
        }
        Event::Panic => panic!("handler panicked"),
        Event::PanicOnExit => {
            cx.ctx_mut().panic_on_exit = true;
            Next::Goto(GotoEdge(Probe::B))
        }
        Event::Spin => {
            cx.raise(Event::Spin);
            Next::Stay
        }
        Event::Hold => {
            note(cx, "held handled".into());
            Next::Stay
        }
        Event::Unknown => Next::Unhandled,
        Event::RaiseUnknown => {
            cx.raise(Event::Unknown);
            Next::Stay
        }
    }
}

fn resolve<S: Sink<()>>(
    next: Next<Probe, GotoEdge>,
    slot: &mut Slot<Probe>,
    cx: &mut ProbeCx<'_, S>,
    policy: UnhandledPolicy,
) -> Outcome {
    match next {
        Next::Stay => Outcome::Stayed,
        Next::Bubble | Next::Unhandled => Outcome::Unhandled(policy),
        Next::Defer => Outcome::Deferred,
        Next::Reenter => {
            let state = *slot.live_mut();
            exit(state, cx);
            entry(state, cx);
            Outcome::Transitioned { changed: false }
        }
        Next::Goto(edge) => Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        },
        Next::__Never(_, never) => match never {},
    }
}

impl Spec for Probe {
    type Event = Event;
    type Effect = ();
    type Ctx = Log;
    type Output = ();
    type Runtime = ProbeRuntime;
    type Effects = InlineEffects<(), 4>;

    const NAME: &'static str = "Probe";
    const GRAPH: &'static Graph = &GRAPH;
    const MAX_MICROSTEPS: u16 = 16;

    fn initial() -> Self {
        Self::A
    }

    fn state_id(&self) -> StateId {
        self.id()
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        path.push(self.id());
        path
    }

    fn enter_initial<S: Sink<()>>(&mut self, cx: &mut ProbeCx<'_, S>) {
        entry(*self, cx);
    }

    fn microstep<S: Sink<()>>(
        slot: &mut Slot<Self>,
        cx: &mut ProbeCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        let state = *slot.live_mut();
        note(cx, format!("{} handles {event:?}", state.name()));
        let (next, policy) = match (state, event) {
            (Probe::A, Event::Hold) => (Next::Defer, Self::UNHANDLED),
            (Probe::B, _) => (handle_common(cx, event), UnhandledPolicy::Error),
            _ => (handle_common(cx, event), Self::UNHANDLED),
        };
        resolve(next, slot, cx, policy)
    }

    fn run_always<S: Sink<()>>(slot: &mut Slot<Self>, cx: &mut ProbeCx<'_, S>) -> Outcome {
        if *slot.live_mut() != Probe::C {
            return Outcome::Stayed;
        }
        note(cx, "always C".into());
        resolve(Next::Goto(GotoEdge(Probe::A)), slot, cx, Self::UNHANDLED)
    }
}
