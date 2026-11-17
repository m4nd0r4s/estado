//! The initial state of the machine comes from `Graph::initial`, not from
//! the declaration order (spec errata 19).

use estado_core::{Graph, StateId, StateInfo, StateKind};
use estado_graph::{to_dot, to_json, to_mermaid, to_scxml};

static STATES: [StateInfo; 3] = [
    StateInfo::new(StateId::new(0), "First", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(1), "Second", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(2), "Third", None, StateKind::Atomic, 0),
];

/// `#[initial]` on the last variant of the root enum.
static LAST_IS_INITIAL: Graph = Graph {
    machine: "Late",
    initial: Some(StateId::new(2)),
    states: &STATES,
    edges: &[],
};

static NO_INITIAL: Graph = Graph {
    machine: "Empty",
    initial: None,
    states: &[],
    edges: &[],
};

#[test]
fn mermaid_points_at_the_declared_initial_state() {
    let diagram = to_mermaid(&LAST_IS_INITIAL);
    assert!(diagram.contains("[*] --> Third\n"), "{diagram}");
    assert!(!diagram.contains("[*] --> First"), "{diagram}");
}

#[test]
fn dot_points_at_the_declared_initial_state() {
    let diagram = to_dot(&LAST_IS_INITIAL);
    assert!(diagram.contains(" -> s2;"), "{diagram}");
    assert!(!diagram.contains(" -> s0;"), "{diagram}");
}

#[test]
fn scxml_names_the_declared_initial_state() {
    let document = to_scxml(&LAST_IS_INITIAL);
    let root = document
        .lines()
        .find(|line| line.starts_with("<scxml"))
        .unwrap();
    assert!(root.contains("initial=\"Third\""), "{root}");
}

#[test]
fn json_records_the_declared_initial_state() {
    assert!(to_json(&LAST_IS_INITIAL).contains("  \"initial\": 2,\n"));
    assert!(to_json(&NO_INITIAL).contains("  \"initial\": null,\n"));
}

#[test]
fn a_graph_without_an_initial_state_draws_no_arrow() {
    assert!(!to_mermaid(&NO_INITIAL).contains("[*]"));
    assert!(!to_scxml(&NO_INITIAL).contains("initial="));
}
