//! Spec 8.3: the worst-case number of entry and exit calls of one
//! transition (ADR 0019), on a hand-built graph with a compound hierarchy
//! and a parallel state.
//!
//! ```text
//! Idle
//! Active (initial Paused)
//!   Playing (initial Normal)
//!     Normal, Fast
//!   Paused
//! Alive (parallel)
//!   movement (region, initial Standing): Standing, Walking
//!   weapon (region, initial Ready): Ready, Reloading
//! Dead (final)
//! ```

use estado_core::{
    EdgeInfo, EdgeKind, EdgeTarget, Graph, HistoryKind, StateId, StateInfo, StateKind,
};

const IDLE: StateId = StateId::new(0);
const ACTIVE: StateId = StateId::new(1);
const PLAYING: StateId = StateId::new(2);
const NORMAL: StateId = StateId::new(3);
const FAST: StateId = StateId::new(4);
const PAUSED: StateId = StateId::new(5);
const ALIVE: StateId = StateId::new(6);
const MOVEMENT: StateId = StateId::new(7);
const STANDING: StateId = StateId::new(8);
const WALKING: StateId = StateId::new(9);
const WEAPON: StateId = StateId::new(10);
const READY: StateId = StateId::new(11);
const RELOADING: StateId = StateId::new(12);
const DEAD: StateId = StateId::new(13);

static STATES: [StateInfo; 14] = [
    StateInfo::new(IDLE, "Idle", None, StateKind::Atomic, 0),
    StateInfo::new(ACTIVE, "Active", None, StateKind::Compound, 0)
        .with_initial(PAUSED)
        .with_history(HistoryKind::Shallow),
    StateInfo::new(PLAYING, "Playing", Some(ACTIVE), StateKind::Compound, 1).with_initial(NORMAL),
    StateInfo::new(NORMAL, "Normal", Some(PLAYING), StateKind::Atomic, 2),
    StateInfo::new(FAST, "Fast", Some(PLAYING), StateKind::Atomic, 2),
    StateInfo::new(PAUSED, "Paused", Some(ACTIVE), StateKind::Atomic, 1),
    StateInfo::new(ALIVE, "Alive", None, StateKind::Parallel, 0),
    StateInfo::new(MOVEMENT, "movement", Some(ALIVE), StateKind::Region, 1).with_initial(STANDING),
    StateInfo::new(STANDING, "Standing", Some(MOVEMENT), StateKind::Atomic, 2),
    StateInfo::new(WALKING, "Walking", Some(MOVEMENT), StateKind::Atomic, 2),
    StateInfo::new(WEAPON, "weapon", Some(ALIVE), StateKind::Region, 1).with_initial(READY),
    StateInfo::new(READY, "Ready", Some(WEAPON), StateKind::Atomic, 2),
    StateInfo::new(RELOADING, "Reloading", Some(WEAPON), StateKind::Atomic, 2),
    StateInfo::new(DEAD, "Dead", None, StateKind::Final, 0),
];

static GRAPH: Graph = Graph {
    machine: "Cost",
    initial: Some(IDLE),
    states: &STATES,
    edges: &[],
};

const fn edge(from: StateId, to: StateId, kind: EdgeKind) -> EdgeInfo {
    EdgeInfo {
        from,
        to: EdgeTarget::State(to),
        kind,
        event_hint: None,
    }
}

fn cost(from: StateId, to: StateId) -> usize {
    GRAPH.edge_actions(&edge(from, to, EdgeKind::Normal))
}

#[test]
fn a_transition_between_top_level_leaves_exits_one_and_enters_one() {
    assert_eq!(cost(IDLE, DEAD), 2);
}

#[test]
fn siblings_share_their_parent() {
    // Normal exits, Fast enters; Playing stays.
    assert_eq!(cost(NORMAL, FAST), 2);
    // Normal and Playing exit, Paused enters; Active stays.
    assert_eq!(cost(NORMAL, PAUSED), 3);
}

#[test]
fn entering_a_compound_state_enters_its_initial_chain() {
    // Idle exits; Active and its initial Paused enter.
    assert_eq!(cost(IDLE, ACTIVE), 3);
    // An explicit target below the initial: Active, Playing, Normal.
    assert_eq!(cost(IDLE, NORMAL), 4);
}

#[test]
fn leaving_a_compound_state_exits_the_whole_active_chain() {
    // Normal, Playing and Active exit; Idle enters.
    assert_eq!(cost(NORMAL, IDLE), 4);
    // From a compound handler the deepest chain below counts: Fast is the
    // same depth as Normal, so Playing, Normal and Active exit.
    assert_eq!(cost(PLAYING, IDLE), 4);
}

#[test]
fn a_local_transition_to_an_ancestor_keeps_the_ancestor() {
    // Normal and Playing exit; the initial of Active (Paused) enters.
    assert_eq!(cost(NORMAL, ACTIVE), 3);
}

#[test]
fn an_external_transition_to_an_ancestor_re_enters_it() {
    let external = GRAPH.edge_actions(&edge(NORMAL, ACTIVE, EdgeKind::External));
    // Normal, Playing and Active exit; Active and Paused enter.
    assert_eq!(external, 5);
}

#[test]
fn a_local_transition_to_a_descendant_keeps_the_source() {
    // Everything below Active exits (Normal, Playing: 2 is the deepest
    // chain); Playing and Fast enter.
    assert_eq!(cost(ACTIVE, FAST), 4);
}

#[test]
fn a_transition_to_the_own_state_exits_and_enters_it_and_its_initials() {
    // Normal and Playing exit, Playing and Normal enter.
    assert_eq!(cost(PLAYING, PLAYING), 4);
    assert_eq!(GRAPH.reenter_actions(PLAYING), 4);
    assert_eq!(GRAPH.reenter_actions(IDLE), 2);
    // The whole parallel state: states and regions' leaves on both sides.
    assert_eq!(GRAPH.reenter_actions(ALIVE), 6);
}

#[test]
fn history_counts_the_largest_configuration_it_may_restore() {
    let plain = GRAPH.edge_actions(&edge(IDLE, ACTIVE, EdgeKind::Normal));
    let history = GRAPH.edge_actions(&edge(IDLE, ACTIVE, EdgeKind::History));
    assert_eq!(plain, 3);
    // Idle exits; Active, Playing and Normal may enter.
    assert_eq!(history, 4);
}

#[test]
fn entering_a_parallel_state_enters_every_region() {
    // Idle exits; Alive, Standing and Ready enter (regions count zero).
    assert_eq!(cost(IDLE, ALIVE), 4);
    // An explicit target inside one region still starts the other.
    assert_eq!(cost(IDLE, WALKING), 4);
}

#[test]
fn leaving_a_parallel_state_exits_every_region() {
    // Walking, Ready (the other region's deepest) and Alive exit; Dead enters.
    assert_eq!(cost(WALKING, DEAD), 4);
    assert_eq!(cost(ALIVE, IDLE), 4);
}

#[test]
fn a_transition_inside_a_region_leaves_the_other_regions_alone() {
    assert_eq!(cost(STANDING, WALKING), 2);
    assert_eq!(cost(READY, RELOADING), 2);
}

#[test]
fn a_target_known_only_at_run_time_counts_the_costliest_state() {
    let dynamic = EdgeInfo {
        from: IDLE,
        to: EdgeTarget::Dynamic,
        kind: EdgeKind::Normal,
        event_hint: None,
    };
    let costliest = STATES
        .iter()
        .filter(|state| !matches!(state.kind, StateKind::Region | StateKind::Choice))
        .map(|state| cost(IDLE, state.id))
        .max()
        .unwrap();
    assert_eq!(GRAPH.edge_actions(&dynamic), costliest);
    assert_eq!(costliest, 4);
    let choice = EdgeInfo {
        to: EdgeTarget::Choice(PAUSED),
        ..dynamic
    };
    assert_eq!(GRAPH.edge_actions(&choice), costliest);
}

#[test]
fn the_worst_case_is_the_largest_edge_and_zero_without_edges() {
    assert_eq!(GRAPH.worst_case_actions(), 0);

    static EDGES: [EdgeInfo; 3] = [
        edge(IDLE, DEAD, EdgeKind::Normal),
        edge(NORMAL, IDLE, EdgeKind::Normal),
        edge(STANDING, WALKING, EdgeKind::Normal),
    ];
    static WITH_EDGES: Graph = Graph {
        machine: "Cost",
        initial: Some(IDLE),
        states: &STATES,
        edges: &EDGES,
    };
    assert_eq!(WITH_EDGES.worst_case_actions(), 4);
}

#[test]
fn the_bound_can_be_evaluated_at_compile_time() {
    static EDGES: [EdgeInfo; 1] = [edge(WALKING, DEAD, EdgeKind::Normal)];
    static WITH_EDGES: Graph = Graph {
        machine: "Cost",
        initial: Some(IDLE),
        states: &STATES,
        edges: &EDGES,
    };
    const BOUND: usize = WITH_EDGES.worst_case_actions();
    const _: () = assert!(BOUND == 4);
    assert_eq!(BOUND, 4);
}
