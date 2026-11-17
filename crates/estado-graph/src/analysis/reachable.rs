//! `reachable` and `dead_ends` (spec 8.3, the basis of W0001 and W0002).

use alloc::vec;
use alloc::vec::Vec;

use estado_core::{EdgeTarget, Graph, StateId, StateKind};

use super::activation::activate;
use crate::tree::Tree;

/// The states that can become active, starting from the initial state of
/// the machine, in declaration order. See [`GraphAnalysis::reachable`].
///
/// [`GraphAnalysis::reachable`]: super::GraphAnalysis::reachable
pub fn reachable(graph: &Graph) -> Vec<StateId> {
    let Some(initial) = graph.initial else {
        return Vec::new();
    };
    let tree = Tree::new(graph);
    let mut reached = vec![false; graph.states.len()];
    activate(&tree, initial.index(), &mut reached);
    loop {
        let before = count(&reached);
        for edge in graph.edges {
            if !reached[edge.from.index()] {
                continue;
            }
            match edge.to {
                EdgeTarget::State(target) | EdgeTarget::Choice(target) => {
                    activate(&tree, target.index(), &mut reached);
                }
                EdgeTarget::Dynamic => {
                    for (index, state) in graph.states.iter().enumerate() {
                        if state.kind != StateKind::Choice {
                            activate(&tree, index, &mut reached);
                        }
                    }
                }
            }
        }
        if count(&reached) == before {
            break;
        }
    }
    graph
        .states
        .iter()
        .zip(&reached)
        .filter(|(_, reached)| **reached)
        .map(|(state, _)| state.id)
        .collect()
}

fn count(flags: &[bool]) -> usize {
    flags.iter().filter(|flag| **flag).count()
}

/// The atomic, non-final states no edge can leave, in declaration order.
/// See [`GraphAnalysis::dead_ends`].
///
/// [`GraphAnalysis::dead_ends`]: super::GraphAnalysis::dead_ends
pub fn dead_ends(graph: &Graph) -> Vec<StateId> {
    let mut declares_edge = vec![false; graph.states.len()];
    for edge in graph.edges {
        declares_edge[edge.from.index()] = true;
    }
    graph
        .states
        .iter()
        .enumerate()
        .filter(|(_, state)| state.kind == StateKind::Atomic)
        .filter(|(index, _)| {
            let mut current = Some(*index);
            while let Some(candidate) = current {
                if declares_edge[candidate] {
                    return false;
                }
                current = graph.states[candidate].parent.map(|parent| parent.index());
            }
            true
        })
        .map(|(_, state)| state.id)
        .collect()
}
