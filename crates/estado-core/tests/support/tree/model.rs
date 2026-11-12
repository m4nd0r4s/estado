//! Types, graph and runtime of the `Tree` test machine.

use estado_core::{
    Graph, HistoryKind, NoRuntime, Runtime, StateId, StateInfo, StateKind, StatePath,
};

/// Root enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tree {
    Idle,
    Stopped { count: u32 },
    Active(Active),
}

/// Compound with deep history (`#[history(deep, keep)]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Active {
    pub count: u32,
    pub sub: ActiveSub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveSub {
    Playing(Playing),
    Paused,
}

/// Compound with shallow history (`#[history]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playing {
    pub speed: u8,
    pub sub: PlayingSub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayingSub {
    Normal,
    Fast,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Handled by `Playing`: goto `Idle` (LCA root).
    PlayingToIdle,
    /// Handled by `Normal` and, from `Fast`, by `Playing`: goto `Paused`
    /// (LCA `Active`).
    ToPaused,
    /// Handled by `Normal`: goto `Fast` (sibling).
    NormalToFast,
    /// Handled by `Active`: goto `Fast` (local, descendant of the handler).
    ActiveToFast,
    /// Handled by `Normal`: goto `Active` (local, ancestor of the handler).
    NormalToActive,
    /// Handled by `Normal`: `goto!(Active, external)`.
    NormalToActiveExternal,
    /// Handled by `Normal`: `goto!(Normal)`, the handler's own state.
    NormalToItself,
    /// Handled by `Playing`: `reenter!()`.
    ReenterPlaying,
    /// Handled by `Playing`: mutate data and stay.
    SetSpeed(u8),
    /// Handled by `Active`: stay.
    StayInActive,
    /// Handled by `Active`: constructor form into `Stopped`.
    ActiveToStopped,
    /// Handled by `Idle` and `Stopped`: `goto!(Active, history)`.
    ToActiveWithHistory,
    /// Handled by `Idle`: plain `goto!(Active)`.
    ToActive,
    /// Handled by `Paused`: `goto!(Playing, history)`.
    PausedToPlayingWithHistory,
}

/// The `Runtime` the macro would generate: history storage only.
pub struct TreeRuntime {
    pub queues: NoRuntime<Event>,
    /// `#[history(deep, keep)]` of `Active`, stored at the root level.
    pub active_history: Option<ActiveSub>,
    /// `#[history]` of `Playing`: discriminant of the last direct substate.
    pub playing_history: Option<u8>,
}

impl Runtime for TreeRuntime {
    type Event = Event;
    type Queues = NoRuntime<Event>;
    const NEW: Self = Self {
        queues: <NoRuntime<Event> as Runtime>::NEW,
        active_history: None,
        playing_history: None,
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }
}

pub const IDLE: StateId = StateId::new(0);
pub const STOPPED: StateId = StateId::new(1);
pub const ACTIVE: StateId = StateId::new(2);
pub const PLAYING: StateId = StateId::new(3);
pub const NORMAL: StateId = StateId::new(4);
pub const FAST: StateId = StateId::new(5);
pub const PAUSED: StateId = StateId::new(6);

static STATES: [StateInfo; 7] = [
    StateInfo::new(IDLE, "Idle", None, StateKind::Atomic, 0),
    StateInfo::new(STOPPED, "Stopped", None, StateKind::Atomic, 0),
    StateInfo::new(ACTIVE, "Active", None, StateKind::Compound, 0)
        .with_initial(PLAYING)
        .with_history(HistoryKind::DeepKeep),
    StateInfo::new(PLAYING, "Playing", Some(ACTIVE), StateKind::Compound, 1)
        .with_initial(NORMAL)
        .with_history(HistoryKind::Shallow),
    StateInfo::new(NORMAL, "Normal", Some(PLAYING), StateKind::Atomic, 2),
    StateInfo::new(FAST, "Fast", Some(PLAYING), StateKind::Atomic, 2),
    StateInfo::new(PAUSED, "Paused", Some(ACTIVE), StateKind::Atomic, 1),
];

pub static GRAPH: Graph = Graph {
    machine: "Tree",
    states: &STATES,
    edges: &[],
};

impl PlayingSub {
    pub fn id(self) -> StateId {
        match self {
            Self::Normal => NORMAL,
            Self::Fast => FAST,
        }
    }

    /// Shallow history discriminant.
    pub fn discriminant(self) -> u8 {
        self as u8
    }

    pub fn from_discriminant(discriminant: u8) -> Self {
        match discriminant {
            0 => Self::Normal,
            _ => Self::Fast,
        }
    }
}

impl Tree {
    pub fn active_path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        match self {
            Self::Idle => path.push(IDLE),
            Self::Stopped { .. } => path.push(STOPPED),
            Self::Active(active) => {
                path.push(ACTIVE);
                match &active.sub {
                    ActiveSub::Playing(playing) => {
                        path.push(PLAYING);
                        path.push(playing.sub.id());
                    }
                    ActiveSub::Paused => path.push(PAUSED),
                }
            }
        }
        path
    }
}

/// Initial value of `Playing` (`#[initial]` data of `ActiveSub`), also used
/// as `#[history_init]` when shallow history rebuilds it.
pub fn initial_playing(sub: PlayingSub) -> Playing {
    Playing { speed: 1, sub }
}
