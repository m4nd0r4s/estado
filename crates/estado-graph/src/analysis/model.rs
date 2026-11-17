//! Which edge can follow which: the model behind `shortest_path` and
//! `edge_cover` (ADR 0019).
//!
//! An edge declared by `from` can be taken whenever `from` is active (a
//! handler of a compound state also fires for its descendants). After an
//! edge, the states it entered are certainly active: the target, its
//! ancestors, the default entry below it and, when it enters a parallel
//! state, the other regions in their initial states. The next edge of a path is any
//! edge whose `from` is among them. The model forgets the states that stay
//! active outside the target (the other regions of a parallel state), which
//! keeps it sound: every path it builds can be walked. It stays complete for
//! covering edges, because such a state was entered by an earlier edge that
//! can start a path of its own.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use estado_core::{EdgeTarget, Graph};

use super::activation::{activate, entered_by};
use crate::tree::Tree;

pub(crate) struct EdgeModel<'graph> {
    tree: Tree<'graph>,
    /// For every edge, the states certainly active after it is taken.
    after: Vec<Vec<bool>>,
    /// For every edge, the edges that can be taken right after it.
    successors: Vec<Vec<usize>>,
}

impl<'graph> EdgeModel<'graph> {
    pub(crate) fn new(graph: &'graph Graph) -> Self {
        let tree = Tree::new(graph);
        let after: Vec<Vec<bool>> = graph
            .edges
            .iter()
            .map(|edge| {
                let mut active = vec![false; graph.states.len()];
                match edge.to {
                    EdgeTarget::State(target) | EdgeTarget::Choice(target) => {
                        entered_by(
                            &tree,
                            edge.from.index(),
                            target.index(),
                            edge.kind,
                            &mut active,
                        );
                    }
                    // Nothing is known about where `goto_dyn!` lands.
                    EdgeTarget::Dynamic => {}
                }
                active
            })
            .collect();
        let successors = after
            .iter()
            .map(|active| {
                (0..graph.edges.len())
                    .filter(|&next| active[graph.edges[next].from.index()])
                    .collect()
            })
            .collect();
        Self {
            tree,
            after,
            successors,
        }
    }

    pub(crate) fn graph(&self) -> &'graph Graph {
        self.tree.graph
    }

    /// The states active when `state` has just been entered.
    pub(crate) fn entered(&self, state: usize) -> Vec<bool> {
        let mut active = vec![false; self.graph().states.len()];
        activate(&self.tree, state, &mut active);
        active
    }

    /// The edges that can be taken when `active` holds, in graph order.
    pub(crate) fn enabled_by(&self, active: &[bool]) -> Vec<usize> {
        (0..self.graph().edges.len())
            .filter(|&edge| active[self.graph().edges[edge].from.index()])
            .collect()
    }

    /// Whether `state` is certainly active after `edge`.
    pub(crate) fn active_after(&self, edge: usize, state: usize) -> bool {
        self.after[edge][state]
    }

    pub(crate) fn successors(&self, edge: usize) -> &[usize] {
        &self.successors[edge]
    }

    /// The shortest walk that starts with one of `sources` and ends with an
    /// edge for which `goal` holds, as edge indices; ties go to the lowest
    /// edge index. `None` when no such walk exists.
    pub(crate) fn search(
        &self,
        sources: &[usize],
        goal: impl Fn(usize) -> bool,
    ) -> Option<Vec<usize>> {
        let count = self.graph().edges.len();
        let mut parent = vec![usize::MAX; count];
        let mut seen = vec![false; count];
        let mut queue = VecDeque::new();
        for &source in sources {
            if !seen[source] {
                seen[source] = true;
                queue.push_back(source);
            }
        }
        while let Some(edge) = queue.pop_front() {
            if goal(edge) {
                let mut walk = vec![edge];
                let mut current = edge;
                while parent[current] != usize::MAX {
                    current = parent[current];
                    walk.push(current);
                }
                walk.reverse();
                return Some(walk);
            }
            for &next in &self.successors[edge] {
                if !seen[next] {
                    seen[next] = true;
                    parent[next] = edge;
                    queue.push_back(next);
                }
            }
        }
        None
    }
}
