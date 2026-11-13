//! State ids, the static graph and `path()` (spec 6.5, 8.1).
//!
//! Regions are states of the graph (`StateKind::Region`), children of the
//! parallel state; their leaves are children of the region.

use estado_core::{
    EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind, StatePath,
};

use super::model::{Conn, Keepalive, Link};

impl Conn {
    /// `Disconnected`.
    pub const DISCONNECTED_ID: StateId = StateId::new(0);
    /// `Connecting`.
    pub const CONNECTING_ID: StateId = StateId::new(1);
    /// `Backoff`.
    pub const BACKOFF_ID: StateId = StateId::new(2);
    /// `Connected`, the parallel state.
    pub const CONNECTED_ID: StateId = StateId::new(3);
    /// Region `link`.
    pub const LINK_ID: StateId = StateId::new(4);
    /// `link:Idle`.
    pub const IDLE_ID: StateId = StateId::new(5);
    /// `link:Sending`.
    pub const SENDING_ID: StateId = StateId::new(6);
    /// Region `keepalive`.
    pub const KEEPALIVE_ID: StateId = StateId::new(7);
    /// `keepalive:Pinging`.
    pub const PINGING_ID: StateId = StateId::new(8);
    /// `keepalive:AwaitingPong`.
    pub const AWAITING_PONG_ID: StateId = StateId::new(9);
    /// `keepalive:Dead`.
    pub const DEAD_ID: StateId = StateId::new(10);
}

static STATES: [StateInfo; 11] = [
    StateInfo::new(
        Conn::DISCONNECTED_ID,
        "Disconnected",
        None,
        StateKind::Atomic,
        0,
    ),
    StateInfo::new(
        Conn::CONNECTING_ID,
        "Connecting",
        None,
        StateKind::Atomic,
        0,
    )
    .with_tasks(1, 1),
    StateInfo::new(Conn::BACKOFF_ID, "Backoff", None, StateKind::Atomic, 0).with_tasks(1, 0),
    StateInfo::new(
        Conn::CONNECTED_ID,
        "Connected",
        None,
        StateKind::Parallel,
        0,
    ),
    StateInfo::new(
        Conn::LINK_ID,
        "link",
        Some(Conn::CONNECTED_ID),
        StateKind::Region,
        1,
    )
    .with_initial(Conn::IDLE_ID),
    StateInfo::new(
        Conn::IDLE_ID,
        "Idle",
        Some(Conn::LINK_ID),
        StateKind::Atomic,
        2,
    ),
    StateInfo::new(
        Conn::SENDING_ID,
        "Sending",
        Some(Conn::LINK_ID),
        StateKind::Atomic,
        2,
    )
    .with_tasks(0, 1),
    StateInfo::new(
        Conn::KEEPALIVE_ID,
        "keepalive",
        Some(Conn::CONNECTED_ID),
        StateKind::Region,
        1,
    )
    .with_initial(Conn::PINGING_ID),
    StateInfo::new(
        Conn::PINGING_ID,
        "Pinging",
        Some(Conn::KEEPALIVE_ID),
        StateKind::Atomic,
        2,
    )
    .with_tasks(1, 0),
    StateInfo::new(
        Conn::AWAITING_PONG_ID,
        "AwaitingPong",
        Some(Conn::KEEPALIVE_ID),
        StateKind::Atomic,
        2,
    )
    .with_tasks(1, 0),
    StateInfo::new(
        Conn::DEAD_ID,
        "Dead",
        Some(Conn::KEEPALIVE_ID),
        StateKind::Atomic,
        2,
    ),
];

const fn edge(from: StateId, to: StateId, kind: EdgeKind, event_hint: &'static str) -> EdgeInfo {
    EdgeInfo {
        from,
        to: EdgeTarget::State(to),
        kind,
        event_hint: Some(event_hint),
    }
}

/// The exact union of the edges of the handlers (spec 6.3).
pub static GRAPH: Graph = Graph {
    machine: "Conn",
    states: &STATES,
    edges: &[
        edge(
            Conn::DISCONNECTED_ID,
            Conn::CONNECTING_ID,
            EdgeKind::Normal,
            "Event::Connect",
        ),
        edge(
            Conn::CONNECTING_ID,
            Conn::CONNECTED_ID,
            EdgeKind::Normal,
            "Event::Connected",
        ),
        edge(
            Conn::CONNECTING_ID,
            Conn::BACKOFF_ID,
            EdgeKind::Timer,
            "Event::ConnectTimeout",
        ),
        edge(
            Conn::CONNECTING_ID,
            Conn::DISCONNECTED_ID,
            EdgeKind::Normal,
            "Event::Disconnect",
        ),
        edge(
            Conn::CONNECTING_ID,
            Conn::DISCONNECTED_ID,
            EdgeKind::Timer,
            "Event::ConnectTimeout",
        ),
        edge(
            Conn::BACKOFF_ID,
            Conn::CONNECTING_ID,
            EdgeKind::Timer,
            "Event::Retry",
        ),
        edge(
            Conn::BACKOFF_ID,
            Conn::DISCONNECTED_ID,
            EdgeKind::Normal,
            "Event::Disconnect",
        ),
        edge(
            Conn::CONNECTED_ID,
            Conn::DISCONNECTED_ID,
            EdgeKind::Normal,
            "Event::Disconnect",
        ),
        edge(
            Conn::IDLE_ID,
            Conn::SENDING_ID,
            EdgeKind::Normal,
            "Event::Send",
        ),
        edge(
            Conn::SENDING_ID,
            Conn::IDLE_ID,
            EdgeKind::Normal,
            "Event::FrameSent",
        ),
        edge(
            Conn::PINGING_ID,
            Conn::AWAITING_PONG_ID,
            EdgeKind::Timer,
            "Event::PingDue",
        ),
        edge(
            Conn::AWAITING_PONG_ID,
            Conn::PINGING_ID,
            EdgeKind::Normal,
            "Event::Pong",
        ),
        edge(
            Conn::AWAITING_PONG_ID,
            Conn::DEAD_ID,
            EdgeKind::Timer,
            "Event::PongTimeout",
        ),
        edge(
            Conn::DEAD_ID,
            Conn::BACKOFF_ID,
            EdgeKind::Normal,
            "Event::KeepaliveDead",
        ),
    ],
};

impl Link {
    /// Id of the active leaf of the region.
    pub fn state_id(&self) -> StateId {
        match self {
            Self::Idle => Conn::IDLE_ID,
            Self::Sending { .. } => Conn::SENDING_ID,
        }
    }
}

impl Keepalive {
    /// Id of the active leaf of the region.
    pub fn state_id(&self) -> StateId {
        match self {
            Self::Pinging => Conn::PINGING_ID,
            Self::AwaitingPong => Conn::AWAITING_PONG_ID,
            Self::Dead => Conn::DEAD_ID,
        }
    }
}

impl Conn {
    /// The active configuration in pre-order, without allocating: the parallel
    /// state, then each region followed by its leaf.
    pub fn active_path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        match self {
            Self::Disconnected => path.push(Self::DISCONNECTED_ID),
            Self::Connecting { .. } => path.push(Self::CONNECTING_ID),
            Self::Backoff { .. } => path.push(Self::BACKOFF_ID),
            Self::Connected(connected) => {
                path.push(Self::CONNECTED_ID);
                path.push(Self::LINK_ID);
                path.push(connected.link.state_id());
                path.push(Self::KEEPALIVE_ID);
                path.push(connected.keepalive.state_id());
            }
        }
        path
    }
}
