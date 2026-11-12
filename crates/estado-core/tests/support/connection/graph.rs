//! The static graph of `Conn` and the ids of its states.
//!
//! Regions are real states of the graph (`StateKind::Region`), children of
//! the parallel state; their leaves are children of the region.

use estado_core::{Graph, StateId, StateInfo, StateKind};

use super::{Conn, DrainSub, IdleSub, Keepalive, Link};

impl Conn {
    pub const IDLE: StateId = StateId::new(0);
    pub const WAITING: StateId = StateId::new(1);
    pub const READY: StateId = StateId::new(2);
    pub const CONNECTED: StateId = StateId::new(3);
    pub const LINK: StateId = StateId::new(4);
    pub const OPENING: StateId = StateId::new(5);
    pub const OPEN: StateId = StateId::new(6);
    pub const FINISHED: StateId = StateId::new(7);
    pub const KEEPALIVE: StateId = StateId::new(8);
    pub const PINGING: StateId = StateId::new(9);
    pub const AWAITING_PONG: StateId = StateId::new(10);
    pub const DEAD: StateId = StateId::new(11);
    pub const STOPPED: StateId = StateId::new(12);
    pub const DRAINING: StateId = StateId::new(13);
    pub const FLUSHING: StateId = StateId::new(14);
    pub const FLUSHED: StateId = StateId::new(15);
    pub const CLOSED: StateId = StateId::new(16);
}

const fn info(
    id: StateId,
    name: &'static str,
    parent: Option<StateId>,
    kind: StateKind,
    depth: u8,
) -> StateInfo {
    StateInfo::new(id, name, parent, kind, depth)
}

static STATES: [StateInfo; 17] = [
    info(Conn::IDLE, "Idle", None, StateKind::Compound, 0).with_initial(Conn::WAITING),
    info(
        Conn::WAITING,
        "Waiting",
        Some(Conn::IDLE),
        StateKind::Atomic,
        1,
    ),
    info(Conn::READY, "Ready", Some(Conn::IDLE), StateKind::Final, 1),
    info(Conn::CONNECTED, "Connected", None, StateKind::Parallel, 0),
    info(
        Conn::LINK,
        "link",
        Some(Conn::CONNECTED),
        StateKind::Region,
        1,
    )
    .with_initial(Conn::OPENING),
    info(
        Conn::OPENING,
        "Opening",
        Some(Conn::LINK),
        StateKind::Atomic,
        2,
    ),
    info(Conn::OPEN, "Open", Some(Conn::LINK), StateKind::Atomic, 2),
    info(
        Conn::FINISHED,
        "Finished",
        Some(Conn::LINK),
        StateKind::Final,
        2,
    ),
    info(
        Conn::KEEPALIVE,
        "keepalive",
        Some(Conn::CONNECTED),
        StateKind::Region,
        1,
    )
    .with_initial(Conn::PINGING),
    info(
        Conn::PINGING,
        "Pinging",
        Some(Conn::KEEPALIVE),
        StateKind::Atomic,
        2,
    ),
    info(
        Conn::AWAITING_PONG,
        "AwaitingPong",
        Some(Conn::KEEPALIVE),
        StateKind::Atomic,
        2,
    ),
    info(
        Conn::DEAD,
        "Dead",
        Some(Conn::KEEPALIVE),
        StateKind::Atomic,
        2,
    ),
    info(
        Conn::STOPPED,
        "Stopped",
        Some(Conn::KEEPALIVE),
        StateKind::Final,
        2,
    ),
    info(Conn::DRAINING, "Draining", None, StateKind::Compound, 0).with_initial(Conn::FLUSHING),
    info(
        Conn::FLUSHING,
        "Flushing",
        Some(Conn::DRAINING),
        StateKind::Atomic,
        1,
    ),
    info(
        Conn::FLUSHED,
        "Flushed",
        Some(Conn::DRAINING),
        StateKind::Final,
        1,
    ),
    info(Conn::CLOSED, "Closed", None, StateKind::Final, 0),
];

pub static GRAPH: Graph = Graph {
    machine: "Conn",
    states: &STATES,
    edges: &[],
};

/// The leaf enums of compound states and regions: what a transition inside
/// them exits and enters.
pub trait Leaf {
    fn state_id(&self) -> StateId;
}

impl Leaf for IdleSub {
    fn state_id(&self) -> StateId {
        match self {
            Self::Waiting => Conn::WAITING,
            Self::Ready(_) => Conn::READY,
        }
    }
}

impl Leaf for Link {
    fn state_id(&self) -> StateId {
        match self {
            Self::Opening => Conn::OPENING,
            Self::Open => Conn::OPEN,
            Self::Finished => Conn::FINISHED,
        }
    }
}

impl Leaf for Keepalive {
    fn state_id(&self) -> StateId {
        match self {
            Self::Pinging => Conn::PINGING,
            Self::AwaitingPong => Conn::AWAITING_PONG,
            Self::Dead => Conn::DEAD,
            Self::Stopped => Conn::STOPPED,
        }
    }
}

impl Leaf for DrainSub {
    fn state_id(&self) -> StateId {
        match self {
            Self::Flushing => Conn::FLUSHING,
            Self::Flushed => Conn::FLUSHED,
        }
    }
}
