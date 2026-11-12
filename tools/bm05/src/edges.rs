//! Plans the `goto!` edges: which handler has which edges, and the events
//! that trigger them. Every transition respects the geometry rules of the
//! macro (no sibling-region transitions, no local transitions of a parallel
//! state, no constructor over a deep history, history only on states that
//! record it), so the generated machine compiles cleanly.

use crate::events::{EVENTS, event_index};
use crate::random::Random;
use crate::tree::{HistoryKind, NodeId, NodeKind, TimerKind, Tree};

/// How an edge is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeForm {
    /// `goto!(Target { .. })`.
    Simple,
    /// `goto!(Target { .. }, history)`.
    History,
    /// `goto!(Target, |old: Exiting![X]| Owner::Target)`.
    Constructor,
}

/// One `goto!`.
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub target: NodeId,
    pub form: EdgeForm,
}

/// One arm of a handler's `match`.
#[derive(Clone, Copy, Debug)]
pub struct Arm {
    pub event: usize,
    /// `None` is a `stay!()` arm.
    pub edge: Option<Edge>,
}

/// The arms of every handler, indexed by node id.
pub struct Plan {
    pub arms: Vec<Vec<Arm>>,
}

impl Plan {
    /// Total number of `goto!` occurrences.
    pub fn edge_count(&self) -> usize {
        self.arms
            .iter()
            .flatten()
            .filter(|arm| arm.edge.is_some())
            .count()
    }
}

/// Plans exactly `edge_budget` edges (at least one ring edge per state).
pub fn plan_edges(tree: &Tree, edge_budget: usize) -> Plan {
    let mut edges: Vec<Vec<Edge>> = vec![Vec::new(); tree.nodes.len()];
    let mut ring_count = 0;
    for owner in 0..tree.nodes.len() {
        ring_count += add_ring_edges(tree, owner, &mut edges);
    }
    assert!(
        ring_count <= edge_budget,
        "the edge budget {edge_budget} does not cover the {ring_count} edges that keep every state reachable"
    );

    let handlers: Vec<NodeId> = tree
        .state_ids()
        .into_iter()
        .filter(|&id| !tree.node(id).terminal)
        .collect();
    let mut random = Random::new(0x5eed_0005);
    for sequence in 0..edge_budget - ring_count {
        let source = handlers[sequence % handlers.len()];
        let form = match sequence % 25 {
            3 | 13 | 21 => EdgeForm::History,
            7 | 17 => EdgeForm::Constructor,
            _ => EdgeForm::Simple,
        };
        let near = sequence % 5 < 3;
        let target = pick_target(tree, source, form, near, &mut random)
            .or_else(|| pick_target(tree, source, EdgeForm::Simple, false, &mut random))
            .expect("every state has a legal simple target");
        let form = if is_legal(tree, source, target, form) {
            form
        } else {
            EdgeForm::Simple
        };
        edges[source].push(Edge { target, form });
    }
    Plan {
        arms: assign_events(tree, &edges),
    }
}

/// A cycle through the non-terminal states of one level, plus an edge from
/// the first of them to each terminal one: everything is reachable and
/// nothing but a terminal state is a dead end. Returns the edges added.
fn add_ring_edges(tree: &Tree, owner: NodeId, edges: &mut [Vec<Edge>]) -> usize {
    let (active, terminals): (Vec<NodeId>, Vec<NodeId>) = tree
        .node(owner)
        .children
        .iter()
        .copied()
        .filter(|&child| tree.is_state(child))
        .partition(|&child| !tree.node(child).terminal);
    let mut added = 0;
    for (index, &source) in active.iter().enumerate() {
        let target = active[(index + 1) % active.len()];
        edges[source].push(Edge {
            target,
            form: EdgeForm::Simple,
        });
        added += 1;
    }
    for &terminal in &terminals {
        edges[active[0]].push(Edge {
            target: terminal,
            form: EdgeForm::Simple,
        });
        added += 1;
    }
    added
}

fn pick_target(
    tree: &Tree,
    source: NodeId,
    form: EdgeForm,
    near: bool,
    random: &mut Random,
) -> Option<NodeId> {
    let candidates: Vec<NodeId> = tree
        .state_ids()
        .into_iter()
        .filter(|&target| is_legal(tree, source, target, form))
        .filter(|&target| {
            !near || tree.depth(tree.transition_lca(source, target)) + 2 >= tree.depth(source)
        })
        .collect();
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[random.below(candidates.len())])
    }
}

/// Whether the macro accepts `goto!` from `source` to `target` in `form`.
pub fn is_legal(tree: &Tree, source: NodeId, target: NodeId, form: EdgeForm) -> bool {
    let lca = tree.transition_lca(source, target);
    if tree.node(lca).kind == NodeKind::Parallel {
        return false;
    }
    match form {
        EdgeForm::Simple => true,
        EdgeForm::History => {
            tree.node(target).history != HistoryKind::None
                && !tree.is_ancestor_or_self(target, source)
                && !tree.is_ancestor_or_self(source, target)
        }
        EdgeForm::Constructor => constructor_exiting_state(tree, source, target).is_some(),
    }
}

/// The state whose value a constructor receives (`Exiting![X]`): the child of
/// the LCA on the path of `source`. The constructor form is only emitted for
/// an atomic target that is a sibling of that state, and never over a deep
/// history (E0090).
pub fn constructor_exiting_state(tree: &Tree, source: NodeId, target: NodeId) -> Option<NodeId> {
    if tree.node(target).kind != NodeKind::Atomic
        || tree.is_ancestor_or_self(target, source)
        || tree.is_ancestor_or_self(source, target)
    {
        return None;
    }
    let lca = tree.transition_lca(source, target);
    if tree.node(target).parent != Some(lca) {
        return None;
    }
    let exiting = std::iter::once(source)
        .chain(tree.ancestors(source))
        .find(|&candidate| tree.node(candidate).parent == Some(lca))?;
    if tree.has_deep_history_at_or_below(exiting) {
        None
    } else {
        Some(exiting)
    }
}

/// Gives each edge of a handler its own event; timer states answer their
/// timer event first, and every third handler also has a `stay!()` arm.
fn assign_events(tree: &Tree, edges: &[Vec<Edge>]) -> Vec<Vec<Arm>> {
    let tick = event_index("Tick");
    let reserved = [event_index("Timeout"), event_index("Deadline"), tick];
    let mut all_arms = Vec::with_capacity(edges.len());
    for (id, handler_edges) in edges.iter().enumerate() {
        let mut arms = Vec::new();
        let mut used = Vec::new();
        let timer_event = match tree.node(id).timer {
            TimerKind::Constant => Some(event_index("Timeout")),
            TimerKind::FromData => Some(event_index("Deadline")),
            TimerKind::None => None,
        };
        let mut cursor = (id * 7 + 3) % EVENTS.len();
        for (position, &edge) in handler_edges.iter().enumerate() {
            let event = match timer_event {
                Some(event) if position == 0 => event,
                _ => loop {
                    cursor = (cursor + 13) % EVENTS.len();
                    if !reserved.contains(&cursor) && !used.contains(&cursor) {
                        break cursor;
                    }
                },
            };
            used.push(event);
            arms.push(Arm {
                event,
                edge: Some(edge),
            });
        }
        if id % 3 == 0 && tree.is_state(id) && !tree.node(id).terminal {
            arms.push(Arm {
                event: tick,
                edge: None,
            });
        }
        all_arms.push(arms);
    }
    all_arms
}
