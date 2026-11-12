//! The two defer overflow policies side by side (spec 5.5).
//!
//! The policy is an associated constant of the `Spec` impl, so a machine has
//! exactly one. Scenario B declares `error` (the right one for a byte stream
//! that must not lose frames); `drop_oldest` has no test in the core, so this
//! probe, a flat machine generic over a const flag, covers both.

use estado_core::{
    Cx, DeferOverflowPolicy, Discard, Graph, Machine, Outcome, Queues, Runtime, SendError, Sink,
    Slot, Spec, StateId, StateInfo, StateKind, StatePath,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Put(u8),
    Open,
}

/// `Closed` defers every `Put`; `Open` delivers it (into the context).
struct Tray<const DROP_OLDEST: bool> {
    open: bool,
}

struct TrayRuntime {
    queues: Queues<Event, 1, 2>,
}

impl Runtime for TrayRuntime {
    type Event = Event;
    type Queues = Queues<Event, 1, 2>;
    const NEW: Self = Self {
        queues: Queues::new(),
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }
}

static STATES: [StateInfo; 2] = [
    StateInfo::new(StateId::new(0), "Closed", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(1), "Open", None, StateKind::Atomic, 0),
];
static GRAPH: Graph = Graph {
    machine: "Tray",
    states: &STATES,
    edges: &[],
};

impl<const DROP_OLDEST: bool> Spec for Tray<DROP_OLDEST> {
    type Event = Event;
    type Effect = ();
    type Ctx = Vec<u8>;
    type Output = ();
    type Runtime = TrayRuntime;
    type Effects = Discard;

    const NAME: &'static str = "Tray";
    const GRAPH: &'static Graph = &GRAPH;
    const DEFER_OVERFLOW: DeferOverflowPolicy = if DROP_OLDEST {
        DeferOverflowPolicy::DropOldest
    } else {
        DeferOverflowPolicy::Error
    };

    fn initial() -> Self {
        Self { open: false }
    }

    fn state_id(&self) -> StateId {
        StateId::new(u16::from(self.open))
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        path.push(self.state_id());
        path
    }

    fn enter_initial<S: Sink<()>>(&mut self, _cx: &mut Cx<'_, Self, S>) {}

    fn microstep<S: Sink<()>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &Event,
    ) -> Outcome {
        let tray = slot.live_mut();
        match (tray.open, event) {
            (false, Event::Put(_)) => Outcome::Deferred,
            (false, Event::Open) => {
                tray.open = true;
                Outcome::Transitioned { changed: true }
            }
            (true, Event::Put(value)) => {
                cx.ctx_mut().push(*value);
                Outcome::Stayed
            }
            (true, Event::Open) => Outcome::Unhandled(Self::UNHANDLED),
        }
    }
}

fn delivered<const DROP_OLDEST: bool>(puts: &[u8]) -> (Vec<Result<(), u8>>, Vec<u8>) {
    let mut machine = Machine::<Tray<DROP_OLDEST>>::new(Vec::new());
    let results = puts
        .iter()
        .map(|value| match machine.send(Event::Put(*value)) {
            Ok(_) => Ok(()),
            Err(SendError::DeferOverflow(Event::Put(refused))) => Err(refused),
            Err(other) => panic!("unexpected error: {other:?}"),
        })
        .collect();
    machine.send(Event::Open).unwrap();
    (results, machine.ctx().clone())
}

#[test]
fn error_refuses_the_event_that_does_not_fit_and_keeps_the_oldest() {
    let (results, delivered) = delivered::<false>(&[1, 2, 3]);
    assert_eq!(results, [Ok(()), Ok(()), Err(3)]);
    assert_eq!(delivered, [1, 2]);
}

#[test]
fn drop_oldest_accepts_everything_and_loses_the_oldest() {
    let (results, delivered) = delivered::<true>(&[1, 2, 3, 4]);
    assert_eq!(results, [Ok(()); 4]);
    assert_eq!(delivered, [3, 4]);
}
