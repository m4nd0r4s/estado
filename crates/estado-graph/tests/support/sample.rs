//! A hand-built graph that uses every kind of state, edge and target, and
//! names that need escaping in every format.

use estado_core::{
    EdgeInfo, EdgeKind, EdgeTarget, Graph, HistoryKind, StateId, StateInfo, StateKind,
};

const fn id(index: u16) -> StateId {
    StateId::new(index)
}

const IDLE: StateId = id(0);
const ACTIVE: StateId = id(1);
const PLAYING: StateId = id(2);
const PAUSED: StateId = id(3);
const NESTED: StateId = id(4);
const INNER: StateId = id(5);
const ALIVE: StateId = id(6);
const LEFT: StateId = id(7);
const LEFT_A: StateId = id(8);
const LEFT_B: StateId = id(9);
const RIGHT: StateId = id(10);
const RIGHT_A: StateId = id(11);
const PICK: StateId = id(12);
const DONE: StateId = id(13);
const FINISHED: StateId = id(14);
const END: StateId = id(15);
const ODD: StateId = id(16);
const END_CLASH: StateId = id(17);

static STATES: [StateInfo; 18] = [
    StateInfo::new(IDLE, "Idle", None, StateKind::Atomic, 0),
    StateInfo::new(ACTIVE, "Active", None, StateKind::Compound, 0)
        .with_initial(PLAYING)
        .with_history(HistoryKind::Shallow),
    StateInfo::new(PLAYING, "Playing", Some(ACTIVE), StateKind::Atomic, 1).with_tasks(1, 1),
    StateInfo::new(PAUSED, "Paused", Some(ACTIVE), StateKind::Atomic, 1),
    StateInfo::new(NESTED, "Nested", Some(ACTIVE), StateKind::Compound, 1)
        .with_initial(INNER)
        .with_history(HistoryKind::DeepKeep),
    StateInfo::new(INNER, "Inner", Some(NESTED), StateKind::Atomic, 2),
    StateInfo::new(ALIVE, "Alive", None, StateKind::Parallel, 0),
    StateInfo::new(LEFT, "left", Some(ALIVE), StateKind::Region, 1).with_initial(LEFT_A),
    StateInfo::new(LEFT_A, "LeftA", Some(LEFT), StateKind::Atomic, 2),
    StateInfo::new(LEFT_B, "LeftB", Some(LEFT), StateKind::Atomic, 2),
    StateInfo::new(RIGHT, "right", Some(ALIVE), StateKind::Region, 1).with_initial(RIGHT_A),
    StateInfo::new(RIGHT_A, "RightA", Some(RIGHT), StateKind::Atomic, 2),
    StateInfo::new(PICK, "Pick", None, StateKind::Choice, 0),
    StateInfo::new(DONE, "Done", None, StateKind::Final, 0),
    StateInfo::new(FINISHED, "Finished", None, StateKind::Final, 0),
    StateInfo::new(END, "end", None, StateKind::Atomic, 0),
    StateInfo::new(ODD, "Odd \"name\" & <tag>", None, StateKind::Atomic, 0),
    StateInfo::new(END_CLASH, "end_15", None, StateKind::Atomic, 0),
];

const fn edge(
    from: StateId,
    to: EdgeTarget,
    kind: EdgeKind,
    event_hint: Option<&'static str>,
) -> EdgeInfo {
    EdgeInfo {
        from,
        to,
        kind,
        event_hint,
    }
}

const fn to(state: StateId) -> EdgeTarget {
    EdgeTarget::State(state)
}

static EDGES: [EdgeInfo; 20] = [
    edge(
        IDLE,
        to(ACTIVE),
        EdgeKind::External,
        Some("Event::Start | Event::Resume"),
    ),
    edge(
        ACTIVE,
        to(IDLE),
        EdgeKind::Normal,
        Some("Event::Stop\n    | Event::Halt"),
    ),
    edge(PLAYING, to(PAUSED), EdgeKind::Normal, Some("Event::Pause")),
    edge(PAUSED, to(PLAYING), EdgeKind::Normal, Some("Event::Play")),
    edge(PLAYING, to(PLAYING), EdgeKind::Timer, Some("Event::Tick")),
    edge(PLAYING, to(NESTED), EdgeKind::Normal, Some("Event::Dive")),
    edge(ACTIVE, to(INNER), EdgeKind::Normal, Some("Event::Zoom")),
    edge(INNER, to(IDLE), EdgeKind::Normal, Some("Event::Surface")),
    edge(IDLE, to(ACTIVE), EdgeKind::History, Some("Event::Continue")),
    edge(IDLE, to(NESTED), EdgeKind::History, Some("Event::Deeper")),
    edge(LEFT_A, to(LEFT_B), EdgeKind::Normal, Some("Event::Left")),
    edge(RIGHT_A, to(IDLE), EdgeKind::Normal, Some("Event::Out")),
    edge(ALIVE, to(DONE), EdgeKind::Done, None),
    edge(IDLE, EdgeTarget::Choice(PICK), EdgeKind::Always, None),
    edge(PICK, to(DONE), EdgeKind::Normal, Some("Event::Yes")),
    edge(PICK, to(FINISHED), EdgeKind::Normal, Some("Event::No")),
    edge(
        PICK,
        EdgeTarget::Dynamic,
        EdgeKind::Normal,
        Some("Event::Any"),
    ),
    edge(DONE, to(IDLE), EdgeKind::Normal, Some("Event::Reset")),
    edge(
        END,
        to(ODD),
        EdgeKind::Error,
        Some("Event::Bad(\"x\") & <y>; \\ done"),
    ),
    edge(ODD, to(END_CLASH), EdgeKind::Normal, None),
];

/// Every state kind, history kind, edge kind and target.
pub static SAMPLE: Graph = Graph {
    machine: "Kitchen \"Sink\"",
    states: &STATES,
    edges: &EDGES,
};
