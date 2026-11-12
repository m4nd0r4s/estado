//! Which states can react while a given state is active, shared by W0003 and
//! W0005.

use std::collections::HashSet;

use crate::model::{StateId, StateKind, StateTree};

/// The states whose handlers can run while `state` is active: the state, its
/// ancestors and the root, everything below it (an event reaches the active
/// leaf first and bubbles up), and, when the state is inside a parallel state,
/// everything below the outermost one (every region sees the event, and a
/// transition in a sibling region changes the configuration too).
pub fn states_in_play(tree: &StateTree, state: StateId) -> HashSet<StateId> {
    let chain = tree.ancestors(state);
    let top = chain
        .iter()
        .copied()
        .rfind(|&node| tree.node(node).kind == StateKind::Parallel)
        .unwrap_or(state);
    let mut in_play: HashSet<StateId> = chain.into_iter().collect();
    in_play.insert(0);
    collect_subtree(tree, top, &mut in_play);
    in_play
}

fn collect_subtree(tree: &StateTree, state: StateId, collected: &mut HashSet<StateId>) {
    collected.insert(state);
    for &child in &tree.node(state).children {
        collect_subtree(tree, child, collected);
    }
}
