//! State ids, the static graph and `path()` (spec 6.5, 8.1).
//!
//! Regions are states of the graph (`StateKind::Region`), children of the
//! parallel state; their leaves are children of the region.

use estado_core::{
    EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind, StatePath,
};

use super::model::{Anim, Character, Movement, Weapon};

impl Character {
    /// `Alive`, the parallel state.
    pub const ALIVE: StateId = StateId::new(0);
    /// Region `movement`.
    pub const MOVEMENT: StateId = StateId::new(1);
    /// `movement:Standing`.
    pub const STANDING: StateId = StateId::new(2);
    /// `movement:Walking`.
    pub const WALKING: StateId = StateId::new(3);
    /// `movement:Jumping`.
    pub const JUMPING: StateId = StateId::new(4);
    /// `movement:Crouching`.
    pub const CROUCHING: StateId = StateId::new(5);
    /// Region `weapon`.
    pub const WEAPON: StateId = StateId::new(6);
    /// `weapon:Ready`.
    pub const READY: StateId = StateId::new(7);
    /// `weapon:Firing`.
    pub const FIRING: StateId = StateId::new(8);
    /// `weapon:Reloading`.
    pub const RELOADING: StateId = StateId::new(9);
    /// `weapon:Empty`.
    pub const EMPTY: StateId = StateId::new(10);
    /// Region `anim`.
    pub const ANIM: StateId = StateId::new(11);
    /// `anim:Idle`.
    pub const IDLE: StateId = StateId::new(12);
    /// `anim:Run`.
    pub const RUN: StateId = StateId::new(13);
    /// `anim:Jump`.
    pub const JUMP: StateId = StateId::new(14);
    /// `anim:Shoot`.
    pub const SHOOT: StateId = StateId::new(15);
    /// `Dead`.
    pub const DEAD: StateId = StateId::new(16);
}

const fn region(id: StateId, name: &'static str, initial: StateId) -> StateInfo {
    StateInfo::new(id, name, Some(Character::ALIVE), StateKind::Region, 1).with_initial(initial)
}

const fn leaf(id: StateId, name: &'static str, region: StateId) -> StateInfo {
    StateInfo::new(id, name, Some(region), StateKind::Atomic, 2)
}

static STATES: [StateInfo; 17] = [
    StateInfo::new(Character::ALIVE, "Alive", None, StateKind::Parallel, 0),
    region(Character::MOVEMENT, "movement", Character::STANDING),
    leaf(Character::STANDING, "Standing", Character::MOVEMENT),
    leaf(Character::WALKING, "Walking", Character::MOVEMENT),
    leaf(Character::JUMPING, "Jumping", Character::MOVEMENT).with_tasks(1, 0),
    leaf(Character::CROUCHING, "Crouching", Character::MOVEMENT),
    region(Character::WEAPON, "weapon", Character::READY),
    leaf(Character::READY, "Ready", Character::WEAPON),
    leaf(Character::FIRING, "Firing", Character::WEAPON).with_tasks(1, 0),
    leaf(Character::RELOADING, "Reloading", Character::WEAPON).with_tasks(1, 0),
    leaf(Character::EMPTY, "Empty", Character::WEAPON),
    region(Character::ANIM, "anim", Character::IDLE),
    leaf(Character::IDLE, "Idle", Character::ANIM),
    leaf(Character::RUN, "Run", Character::ANIM),
    leaf(Character::JUMP, "Jump", Character::ANIM),
    leaf(Character::SHOOT, "Shoot", Character::ANIM),
    StateInfo::new(Character::DEAD, "Dead", None, StateKind::Atomic, 0),
];

const fn edge(from: StateId, to: StateId, kind: EdgeKind, event_hint: &'static str) -> EdgeInfo {
    EdgeInfo {
        from,
        to: EdgeTarget::State(to),
        kind,
        event_hint: Some(event_hint),
    }
}

const fn normal(from: StateId, to: StateId, event_hint: &'static str) -> EdgeInfo {
    edge(from, to, EdgeKind::Normal, event_hint)
}

const fn timer(from: StateId, to: StateId, event_hint: &'static str) -> EdgeInfo {
    edge(from, to, EdgeKind::Timer, event_hint)
}

/// The events after which a pose handler of `anim` re-derives the pose; each
/// of those handlers has a `goto!` to every other pose.
const POSE: &str = "Move | Stop | Jump | Land | Crouch | Stand | Fire | CooldownElapsed";

/// The exact union of the edges of the handlers (spec 6.3).
pub static GRAPH: Graph = Graph {
    machine: "Character",
    states: &STATES,
    edges: &[
        normal(Character::ALIVE, Character::DEAD, "Event::Damage"),
        normal(Character::DEAD, Character::ALIVE, "Event::Respawn"),
        normal(Character::STANDING, Character::WALKING, "Event::Move"),
        normal(Character::STANDING, Character::JUMPING, "Event::Jump"),
        normal(Character::STANDING, Character::CROUCHING, "Event::Crouch"),
        normal(Character::WALKING, Character::STANDING, "Event::Stop"),
        normal(Character::WALKING, Character::JUMPING, "Event::Jump"),
        normal(Character::WALKING, Character::CROUCHING, "Event::Crouch"),
        timer(Character::JUMPING, Character::STANDING, "Event::Land"),
        normal(Character::CROUCHING, Character::STANDING, "Event::Stand"),
        normal(Character::READY, Character::FIRING, "Event::Fire"),
        normal(
            Character::READY,
            Character::RELOADING,
            "Event::ReloadRequest",
        ),
        timer(
            Character::FIRING,
            Character::READY,
            "Event::CooldownElapsed",
        ),
        timer(
            Character::FIRING,
            Character::EMPTY,
            "Event::CooldownElapsed",
        ),
        timer(Character::RELOADING, Character::READY, "Event::ReloadDone"),
        normal(
            Character::EMPTY,
            Character::RELOADING,
            "Event::ReloadRequest",
        ),
        normal(Character::IDLE, Character::RUN, POSE),
        normal(Character::IDLE, Character::JUMP, POSE),
        normal(Character::IDLE, Character::SHOOT, POSE),
        normal(Character::RUN, Character::IDLE, POSE),
        normal(Character::RUN, Character::JUMP, POSE),
        normal(Character::RUN, Character::SHOOT, POSE),
        normal(Character::JUMP, Character::IDLE, POSE),
        normal(Character::JUMP, Character::RUN, POSE),
        normal(Character::JUMP, Character::SHOOT, POSE),
        normal(Character::SHOOT, Character::IDLE, POSE),
        normal(Character::SHOOT, Character::RUN, POSE),
        normal(Character::SHOOT, Character::JUMP, POSE),
    ],
};

impl Movement {
    /// Id of the active leaf of the region.
    pub fn state_id(self) -> StateId {
        match self {
            Self::Standing => Character::STANDING,
            Self::Walking => Character::WALKING,
            Self::Jumping => Character::JUMPING,
            Self::Crouching => Character::CROUCHING,
        }
    }
}

impl Weapon {
    /// Id of the active leaf of the region.
    pub fn state_id(self) -> StateId {
        match self {
            Self::Ready => Character::READY,
            Self::Firing => Character::FIRING,
            Self::Reloading => Character::RELOADING,
            Self::Empty => Character::EMPTY,
        }
    }
}

impl Anim {
    /// Id of the active leaf of the region.
    pub fn state_id(self) -> StateId {
        match self {
            Self::Idle => Character::IDLE,
            Self::Run => Character::RUN,
            Self::Jump => Character::JUMP,
            Self::Shoot => Character::SHOOT,
        }
    }
}

impl Character {
    /// The active configuration in pre-order, without allocating: the parallel
    /// state, then each region followed by its leaf.
    pub fn active_path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        match self {
            Self::Alive(alive) => {
                path.push(Self::ALIVE);
                path.push(Self::MOVEMENT);
                path.push(alive.movement.state_id());
                path.push(Self::WEAPON);
                path.push(alive.weapon.state_id());
                path.push(Self::ANIM);
                path.push(alive.anim.state_id());
            }
            Self::Dead => path.push(Self::DEAD),
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use estado_core::Edge;

    use super::*;
    use crate::d_game_character::anim_edges::{IdleEdges, JumpEdges, RunEdges, ShootEdges};
    use crate::d_game_character::edges::{AliveEdges, DeadEdges};
    use crate::d_game_character::movement_edges::{
        CrouchingEdges, JumpingEdges, StandingEdges, WalkingEdges,
    };
    use crate::d_game_character::weapon_edges::{
        EmptyEdges, FiringEdges, ReadyEdges, ReloadingEdges,
    };

    /// The graph is the exact union of the edge enums (spec 6.3).
    #[test]
    fn graph_edges_match_the_targets_of_the_edge_enums() {
        let declared: [(StateId, &[StateId]); 16] = [
            (Character::ALIVE, AliveEdges::TARGETS),
            (Character::DEAD, DeadEdges::TARGETS),
            (Character::STANDING, StandingEdges::TARGETS),
            (Character::WALKING, WalkingEdges::TARGETS),
            (Character::JUMPING, JumpingEdges::TARGETS),
            (Character::CROUCHING, CrouchingEdges::TARGETS),
            (Character::READY, ReadyEdges::TARGETS),
            (Character::FIRING, FiringEdges::TARGETS),
            (Character::RELOADING, ReloadingEdges::TARGETS),
            (Character::EMPTY, EmptyEdges::TARGETS),
            (Character::IDLE, IdleEdges::TARGETS),
            (Character::RUN, RunEdges::TARGETS),
            (Character::JUMP, JumpEdges::TARGETS),
            (Character::SHOOT, ShootEdges::TARGETS),
            (Character::ANIM, &[]),
            (Character::MOVEMENT, &[]),
        ];
        for (from, targets) in declared {
            let in_graph: Vec<_> = GRAPH
                .edges
                .iter()
                .filter(|edge| edge.from == from)
                .map(|edge| edge.to)
                .collect();
            let in_enum: Vec<_> = targets.iter().copied().map(EdgeTarget::State).collect();
            assert_eq!(in_graph, in_enum, "edges from {from:?}");
        }
        assert_eq!(GRAPH.edges.len(), 28, "no edge is left out of the check");
    }

    /// State ids are indexes into the graph, and the timers are declared.
    #[test]
    fn state_ids_index_the_graph_and_timers_are_declared() {
        for (index, info) in GRAPH.states.iter().enumerate() {
            assert_eq!(info.id.index(), index, "{}", info.name);
        }
        let with_timer: Vec<_> = GRAPH
            .states
            .iter()
            .filter(|info| info.timers > 0)
            .map(|info| info.id)
            .collect();
        assert_eq!(
            with_timer,
            [Character::JUMPING, Character::FIRING, Character::RELOADING]
        );
    }
}
