//! W0001 (unreachable state) and W0002 (dead end), computed from the state
//! tree and the edges between states.
//!
//! The edges come from the handler rewrite: one [`EdgeSource`] per handler,
//! listing the targets of its `goto!` occurrences ([`edge_sources`]).

use std::collections::HashSet;

use super::code::WarningCode;
use super::emit::Warning;
use crate::model::{MachineModel, StateId, StateKind, StateTree};

/// The `goto!` targets of one handler.
pub struct EdgeSource {
    /// The state the handler belongs to (it also fires for descendants).
    pub from: StateId,
    /// Target of each `goto!` of the handler.
    pub targets: Vec<StateId>,
    /// The handler uses `goto_dyn!`: the target is only known at run time.
    pub has_dynamic_target: bool,
}

/// One [`EdgeSource`] per analysed handler, from its `goto!` occurrences.
/// Entries, exits and handlers without a `goto!` give an empty source, which
/// is harmless. `goto_dyn!` is not supported yet, so none is dynamic.
pub fn edge_sources(model: &MachineModel) -> Vec<EdgeSource> {
    model
        .analysed_handlers
        .iter()
        .map(|handler| EdgeSource {
            from: handler.state,
            targets: handler.gotos.iter().map(|goto| goto.target).collect(),
            has_dynamic_target: false,
        })
        .collect()
}

/// The W0001 and W0002 warnings of a machine, given all its edges.
///
/// Call this only with the complete edge list of the machine: with missing
/// edges every state would look unreachable.
pub fn graph_warnings(model: &MachineModel, edges: &[EdgeSource]) -> Vec<Warning> {
    let tree = &model.tree;
    let unreachable = unreachable_states(tree, edges).into_iter().map(|id| {
        let name = &tree.node(id).name;
        Warning {
            code: WarningCode::UnreachableState,
            span: name.span(),
            subject: crate::naming::plain_name(name),
            message: format!("state `{name}` is unreachable from the initial state"),
            condition: None,
        }
    });
    let dead_ends = dead_end_states(tree, edges).into_iter().map(|id| {
        let name = &tree.node(id).name;
        Warning {
            code: WarningCode::DeadEnd,
            span: name.span(),
            subject: crate::naming::plain_name(name),
            message: format!(
                "state `{name}` has no outgoing edges and is not `#[terminal]`; \
                 once entered the machine can never leave it"
            ),
            condition: None,
        }
    });
    unreachable.chain(dead_ends).collect()
}

/// States that can never become active, reported once at the topmost one
/// (descendants of an unreachable state are not repeated).
///
/// A state is reachable when it is part of the initial configuration or the
/// target of an edge whose source is reachable. Entering a state also enters
/// its ancestors and the initial descendants. A `goto_dyn!` can reach any
/// state, so with one present nothing is reported (no false positives).
pub fn unreachable_states(tree: &StateTree, edges: &[EdgeSource]) -> Vec<StateId> {
    if edges.iter().any(|source| source.has_dynamic_target) {
        return Vec::new();
    }
    let Some(root_initial) = tree.root().initial else {
        return Vec::new(); // E0010 already reports it
    };
    let mut reached = HashSet::new();
    activate(tree, root_initial, &mut reached);
    loop {
        let before = reached.len();
        for source in edges {
            if reached.contains(&source.from) || source.from == 0 {
                for &target in &source.targets {
                    activate(tree, target, &mut reached);
                }
            }
        }
        if reached.len() == before {
            break;
        }
    }
    tree.nodes
        .iter()
        .skip(1)
        .filter(|node| !reached.contains(&node.id))
        .filter(|node| {
            node.parent
                .is_none_or(|parent| parent == 0 || reached.contains(&parent))
        })
        .map(|node| node.id)
        .collect()
}

/// Leaf states from which no edge can leave: no handler of the state or of
/// one of its ancestors has a `goto!`. `#[terminal]` states are expected to
/// be dead ends. Compound and parallel states are not reported, only the
/// leaves; a region leaf counts like any other leaf.
pub fn dead_end_states(tree: &StateTree, edges: &[EdgeSource]) -> Vec<StateId> {
    let sources_with_edges: HashSet<StateId> = edges
        .iter()
        .filter(|source| !source.targets.is_empty() || source.has_dynamic_target)
        .map(|source| source.from)
        .collect();
    tree.nodes
        .iter()
        .skip(1)
        .filter(|node| node.kind == StateKind::Atomic && !node.is_final)
        .filter(|node| {
            let can_leave = sources_with_edges.contains(&0)
                || tree
                    .ancestors(node.id)
                    .iter()
                    .any(|ancestor| sources_with_edges.contains(ancestor));
            !can_leave
        })
        .map(|node| node.id)
        .collect()
}

/// Marks `state` active: it, its ancestors, and what entering it activates by
/// default (the initial substate, every region and its initial state).
fn activate(tree: &StateTree, state: StateId, reached: &mut HashSet<StateId>) {
    if state == 0 {
        return;
    }
    reached.extend(tree.ancestors(state));
    activate_defaults(tree, state, reached);
}

fn activate_defaults(tree: &StateTree, state: StateId, reached: &mut HashSet<StateId>) {
    let node = tree.node(state);
    match node.kind {
        StateKind::Parallel => {
            for &region in &node.children {
                reached.insert(region);
                activate_defaults(tree, region, reached);
            }
        }
        StateKind::Compound | StateKind::Region => {
            if let Some(initial) = node.initial {
                reached.insert(initial);
                activate_defaults(tree, initial, reached);
            }
        }
        StateKind::Atomic | StateKind::Root => {}
    }
}
