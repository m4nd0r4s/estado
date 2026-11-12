//! State ids, the static graph and `path()` (spec 6.5, 8.1).

use estado_core::{
    EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind, StatePath,
};

use super::model::{ActiveSub, Player};

impl Player {
    /// `Idle`.
    pub const IDLE: StateId = StateId::new(0);
    /// `Active`.
    pub const ACTIVE: StateId = StateId::new(1);
    /// `Active/Playing`.
    pub const PLAYING: StateId = StateId::new(2);
    /// `Active/Paused`.
    pub const PAUSED: StateId = StateId::new(3);
    /// `Failed`.
    pub const FAILED: StateId = StateId::new(4);
}

static STATES: [StateInfo; 5] = [
    StateInfo::new(Player::IDLE, "Idle", None, StateKind::Atomic, 0),
    StateInfo::new(Player::ACTIVE, "Active", None, StateKind::Compound, 0)
        .with_initial(Player::PLAYING),
    StateInfo::new(
        Player::PLAYING,
        "Playing",
        Some(Player::ACTIVE),
        StateKind::Atomic,
        1,
    ),
    StateInfo::new(
        Player::PAUSED,
        "Paused",
        Some(Player::ACTIVE),
        StateKind::Atomic,
        1,
    ),
    StateInfo::new(Player::FAILED, "Failed", None, StateKind::Atomic, 0),
];

const fn edge(from: StateId, to: StateId, event_hint: &'static str) -> EdgeInfo {
    EdgeInfo {
        from,
        to: EdgeTarget::State(to),
        kind: EdgeKind::Normal,
        event_hint: Some(event_hint),
    }
}

/// The exact union of the edges of the handlers (spec 6.3).
pub static GRAPH: Graph = Graph {
    machine: "Player",
    states: &STATES,
    edges: &[
        edge(Player::IDLE, Player::ACTIVE, "Event::Play"),
        edge(Player::PLAYING, Player::PAUSED, "Event::Pause"),
        edge(Player::PAUSED, Player::PLAYING, "Event::Resume"),
        edge(Player::ACTIVE, Player::IDLE, "Event::Stop"),
        edge(Player::ACTIVE, Player::ACTIVE, "Event::Play"),
        edge(Player::ACTIVE, Player::FAILED, "Event::Error"),
        edge(Player::FAILED, Player::ACTIVE, "Event::Play"),
        edge(Player::FAILED, Player::IDLE, "Event::Stop"),
    ],
};

impl Player {
    /// The active configuration, without allocating.
    pub fn active_path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        match self {
            Self::Idle => path.push(Self::IDLE),
            Self::Active(active) => {
                path.push(Self::ACTIVE);
                path.push(match active.sub {
                    ActiveSub::Playing { .. } => Self::PLAYING,
                    ActiveSub::Paused { .. } => Self::PAUSED,
                });
            }
            Self::Failed { .. } => path.push(Self::FAILED),
        }
        path
    }
}
