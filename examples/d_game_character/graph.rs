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
    pub const ALIVE_ID: StateId = StateId::new(0);
    /// Region `movement`.
    pub const MOVEMENT_ID: StateId = StateId::new(1);
    /// `movement:Standing`.
    pub const STANDING_ID: StateId = StateId::new(2);
    /// `movement:Walking`.
    pub const WALKING_ID: StateId = StateId::new(3);
    /// `movement:Jumping`.
    pub const JUMPING_ID: StateId = StateId::new(4);
    /// `movement:Crouching`.
    pub const CROUCHING_ID: StateId = StateId::new(5);
    /// Region `weapon`.
    pub const WEAPON_ID: StateId = StateId::new(6);
    /// `weapon:Ready`.
    pub const READY_ID: StateId = StateId::new(7);
    /// `weapon:Firing`.
    pub const FIRING_ID: StateId = StateId::new(8);
    /// `weapon:Reloading`.
    pub const RELOADING_ID: StateId = StateId::new(9);
    /// `weapon:Empty`.
    pub const EMPTY_ID: StateId = StateId::new(10);
    /// Region `anim`.
    pub const ANIM_ID: StateId = StateId::new(11);
    /// `anim:Idle`.
    pub const IDLE_ID: StateId = StateId::new(12);
    /// `anim:Run`.
    pub const RUN_ID: StateId = StateId::new(13);
    /// `anim:Jump`.
    pub const JUMP_ID: StateId = StateId::new(14);
    /// `anim:Shoot`.
    pub const SHOOT_ID: StateId = StateId::new(15);
    /// `Dead`.
    pub const DEAD_ID: StateId = StateId::new(16);
}

const fn region(id: StateId, name: &'static str, initial: StateId) -> StateInfo {
    StateInfo::new(id, name, Some(Character::ALIVE_ID), StateKind::Region, 1).with_initial(initial)
}

const fn leaf(id: StateId, name: &'static str, region: StateId) -> StateInfo {
    StateInfo::new(id, name, Some(region), StateKind::Atomic, 2)
}

static STATES: [StateInfo; 17] = [
    StateInfo::new(Character::ALIVE_ID, "Alive", None, StateKind::Parallel, 0),
    region(Character::MOVEMENT_ID, "movement", Character::STANDING_ID),
    leaf(Character::STANDING_ID, "Standing", Character::MOVEMENT_ID),
    leaf(Character::WALKING_ID, "Walking", Character::MOVEMENT_ID),
    leaf(Character::JUMPING_ID, "Jumping", Character::MOVEMENT_ID).with_tasks(1, 0),
    leaf(Character::CROUCHING_ID, "Crouching", Character::MOVEMENT_ID),
    region(Character::WEAPON_ID, "weapon", Character::READY_ID),
    leaf(Character::READY_ID, "Ready", Character::WEAPON_ID),
    leaf(Character::FIRING_ID, "Firing", Character::WEAPON_ID).with_tasks(1, 0),
    leaf(Character::RELOADING_ID, "Reloading", Character::WEAPON_ID).with_tasks(1, 0),
    leaf(Character::EMPTY_ID, "Empty", Character::WEAPON_ID),
    region(Character::ANIM_ID, "anim", Character::IDLE_ID),
    leaf(Character::IDLE_ID, "Idle", Character::ANIM_ID),
    leaf(Character::RUN_ID, "Run", Character::ANIM_ID),
    leaf(Character::JUMP_ID, "Jump", Character::ANIM_ID),
    leaf(Character::SHOOT_ID, "Shoot", Character::ANIM_ID),
    StateInfo::new(Character::DEAD_ID, "Dead", None, StateKind::Atomic, 0),
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
        normal(Character::ALIVE_ID, Character::DEAD_ID, "Event::Damage"),
        normal(Character::DEAD_ID, Character::ALIVE_ID, "Event::Respawn"),
        normal(Character::STANDING_ID, Character::WALKING_ID, "Event::Move"),
        normal(Character::STANDING_ID, Character::JUMPING_ID, "Event::Jump"),
        normal(
            Character::STANDING_ID,
            Character::CROUCHING_ID,
            "Event::Crouch",
        ),
        normal(Character::WALKING_ID, Character::STANDING_ID, "Event::Stop"),
        normal(Character::WALKING_ID, Character::JUMPING_ID, "Event::Jump"),
        normal(
            Character::WALKING_ID,
            Character::CROUCHING_ID,
            "Event::Crouch",
        ),
        timer(Character::JUMPING_ID, Character::STANDING_ID, "Event::Land"),
        normal(
            Character::CROUCHING_ID,
            Character::STANDING_ID,
            "Event::Stand",
        ),
        normal(Character::READY_ID, Character::FIRING_ID, "Event::Fire"),
        normal(
            Character::READY_ID,
            Character::RELOADING_ID,
            "Event::ReloadRequest",
        ),
        timer(
            Character::FIRING_ID,
            Character::READY_ID,
            "Event::CooldownElapsed",
        ),
        timer(
            Character::FIRING_ID,
            Character::EMPTY_ID,
            "Event::CooldownElapsed",
        ),
        timer(
            Character::RELOADING_ID,
            Character::READY_ID,
            "Event::ReloadDone",
        ),
        normal(
            Character::EMPTY_ID,
            Character::RELOADING_ID,
            "Event::ReloadRequest",
        ),
        normal(Character::IDLE_ID, Character::RUN_ID, POSE),
        normal(Character::IDLE_ID, Character::JUMP_ID, POSE),
        normal(Character::IDLE_ID, Character::SHOOT_ID, POSE),
        normal(Character::RUN_ID, Character::IDLE_ID, POSE),
        normal(Character::RUN_ID, Character::JUMP_ID, POSE),
        normal(Character::RUN_ID, Character::SHOOT_ID, POSE),
        normal(Character::JUMP_ID, Character::IDLE_ID, POSE),
        normal(Character::JUMP_ID, Character::RUN_ID, POSE),
        normal(Character::JUMP_ID, Character::SHOOT_ID, POSE),
        normal(Character::SHOOT_ID, Character::IDLE_ID, POSE),
        normal(Character::SHOOT_ID, Character::RUN_ID, POSE),
        normal(Character::SHOOT_ID, Character::JUMP_ID, POSE),
    ],
};

impl Movement {
    /// Id of the active leaf of the region.
    pub fn state_id(self) -> StateId {
        match self {
            Self::Standing => Character::STANDING_ID,
            Self::Walking => Character::WALKING_ID,
            Self::Jumping => Character::JUMPING_ID,
            Self::Crouching => Character::CROUCHING_ID,
        }
    }
}

impl Weapon {
    /// Id of the active leaf of the region.
    pub fn state_id(self) -> StateId {
        match self {
            Self::Ready => Character::READY_ID,
            Self::Firing => Character::FIRING_ID,
            Self::Reloading => Character::RELOADING_ID,
            Self::Empty => Character::EMPTY_ID,
        }
    }
}

impl Anim {
    /// Id of the active leaf of the region.
    pub fn state_id(self) -> StateId {
        match self {
            Self::Idle => Character::IDLE_ID,
            Self::Run => Character::RUN_ID,
            Self::Jump => Character::JUMP_ID,
            Self::Shoot => Character::SHOOT_ID,
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
                path.push(Self::ALIVE_ID);
                path.push(Self::MOVEMENT_ID);
                path.push(alive.movement.state_id());
                path.push(Self::WEAPON_ID);
                path.push(alive.weapon.state_id());
                path.push(Self::ANIM_ID);
                path.push(alive.anim.state_id());
            }
            Self::Dead => path.push(Self::DEAD_ID),
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
            (Character::ALIVE_ID, AliveEdges::TARGETS),
            (Character::DEAD_ID, DeadEdges::TARGETS),
            (Character::STANDING_ID, StandingEdges::TARGETS),
            (Character::WALKING_ID, WalkingEdges::TARGETS),
            (Character::JUMPING_ID, JumpingEdges::TARGETS),
            (Character::CROUCHING_ID, CrouchingEdges::TARGETS),
            (Character::READY_ID, ReadyEdges::TARGETS),
            (Character::FIRING_ID, FiringEdges::TARGETS),
            (Character::RELOADING_ID, ReloadingEdges::TARGETS),
            (Character::EMPTY_ID, EmptyEdges::TARGETS),
            (Character::IDLE_ID, IdleEdges::TARGETS),
            (Character::RUN_ID, RunEdges::TARGETS),
            (Character::JUMP_ID, JumpEdges::TARGETS),
            (Character::SHOOT_ID, ShootEdges::TARGETS),
            (Character::ANIM_ID, &[]),
            (Character::MOVEMENT_ID, &[]),
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
            [
                Character::JUMPING_ID,
                Character::FIRING_ID,
                Character::RELOADING_ID
            ]
        );
    }
}
