//! State ids, the static graph and `path()` (spec 6.5, 8.1).

use estado_core::{
    EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind, StatePath,
};

use super::model::{ActiveSub, Player};

impl Player {
    /// `Idle`.
    pub const IDLE_ID: StateId = StateId::new(0);
    /// `Active`.
    pub const ACTIVE_ID: StateId = StateId::new(1);
    /// `Active/Playing`.
    pub const PLAYING_ID: StateId = StateId::new(2);
    /// `Active/Paused`.
    pub const PAUSED_ID: StateId = StateId::new(3);
    /// `Failed`.
    pub const FAILED_ID: StateId = StateId::new(4);
}

static STATES: [StateInfo; 5] = [
    StateInfo::new(Player::IDLE_ID, "Idle", None, StateKind::Atomic, 0),
    StateInfo::new(Player::ACTIVE_ID, "Active", None, StateKind::Compound, 0)
        .with_initial(Player::PLAYING_ID),
    StateInfo::new(
        Player::PLAYING_ID,
        "Playing",
        Some(Player::ACTIVE_ID),
        StateKind::Atomic,
        1,
    ),
    StateInfo::new(
        Player::PAUSED_ID,
        "Paused",
        Some(Player::ACTIVE_ID),
        StateKind::Atomic,
        1,
    ),
    StateInfo::new(Player::FAILED_ID, "Failed", None, StateKind::Atomic, 0),
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
        edge(Player::IDLE_ID, Player::ACTIVE_ID, "Event::Play"),
        edge(Player::PLAYING_ID, Player::PAUSED_ID, "Event::Pause"),
        edge(Player::PAUSED_ID, Player::PLAYING_ID, "Event::Resume"),
        edge(Player::ACTIVE_ID, Player::IDLE_ID, "Event::Stop"),
        edge(Player::ACTIVE_ID, Player::ACTIVE_ID, "Event::Play"),
        edge(Player::ACTIVE_ID, Player::FAILED_ID, "Event::Error"),
        edge(Player::FAILED_ID, Player::ACTIVE_ID, "Event::Play"),
        edge(Player::FAILED_ID, Player::IDLE_ID, "Event::Stop"),
    ],
};

impl Player {
    /// The active configuration, without allocating.
    pub fn active_path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        match self {
            Self::Idle => path.push(Self::IDLE_ID),
            Self::Active(active) => {
                path.push(Self::ACTIVE_ID);
                path.push(match active.sub {
                    ActiveSub::Playing { .. } => Self::PLAYING_ID,
                    ActiveSub::Paused { .. } => Self::PAUSED_ID,
                });
            }
            Self::Failed { .. } => path.push(Self::FAILED_ID),
        }
        path
    }
}
