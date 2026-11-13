//! The parent/child structure of a [`Graph`], shared by the exporters.

use alloc::vec::Vec;

use estado_core::{EdgeInfo, EdgeTarget, Graph, StateKind};

/// Children lists of a graph, in declaration order.
///
/// Every index is a position in [`Graph::states`], which is also the
/// [`StateId::index`](estado_core::StateId::index) of the state.
pub(crate) struct Tree<'graph> {
    pub(crate) graph: &'graph Graph,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
}

impl<'graph> Tree<'graph> {
    pub(crate) fn new(graph: &'graph Graph) -> Self {
        let mut children = alloc::vec![Vec::new(); graph.states.len()];
        let mut roots = Vec::new();
        for (index, state) in graph.states.iter().enumerate() {
            match state.parent {
                Some(parent) => children[parent.index()].push(index),
                None => roots.push(index),
            }
        }
        Self {
            graph,
            children,
            roots,
        }
    }

    /// The top-level states.
    pub(crate) fn roots(&self) -> &[usize] {
        &self.roots
    }

    /// The direct children of `state`.
    pub(crate) fn children(&self, state: usize) -> &[usize] {
        &self.children[state]
    }

    pub(crate) fn kind(&self, state: usize) -> StateKind {
        self.graph.states[state].kind
    }

    pub(crate) fn parent(&self, state: usize) -> Option<usize> {
        self.graph.states[state].parent.map(|parent| parent.index())
    }

    /// The initial state of the machine: the first top-level state.
    ///
    /// The graph does not record the root `#[initial]` (spec errata 19), so
    /// this is the best the exporters can say.
    pub(crate) fn machine_initial(&self) -> Option<usize> {
        self.roots.first().copied()
    }

    /// Whether `ancestor` is a strict ancestor of `state`.
    pub(crate) fn is_strict_ancestor(&self, ancestor: usize, state: usize) -> bool {
        let mut current = self.parent(state);
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            current = self.parent(parent);
        }
        false
    }

    /// The lowest state that is an ancestor-or-self of both, if any.
    fn lowest_common_ancestor(&self, left: usize, right: usize) -> Option<usize> {
        let mut current = Some(left);
        while let Some(candidate) = current {
            if candidate == right || self.is_strict_ancestor(candidate, right) {
                return Some(candidate);
            }
            current = self.parent(candidate);
        }
        None
    }

    /// The scope a diagram nests the edge in: the nearest compound state or
    /// region that contains both ends, strictly above them, or `None` for the
    /// top level.
    ///
    /// A parallel state is not a scope of its own (its regions are), and an
    /// edge between a state and its own descendant belongs outside that state.
    pub(crate) fn edge_scope(&self, edge: &EdgeInfo) -> Option<usize> {
        let from = edge.from.index();
        let target = match edge.to {
            EdgeTarget::State(state) | EdgeTarget::Choice(state) => Some(state.index()),
            EdgeTarget::Dynamic => None,
        };
        let meeting = match target {
            Some(target) => self.lowest_common_ancestor(from, target),
            None => Some(from),
        };
        let mut candidate = match (meeting, target) {
            (Some(meeting), Some(target)) if meeting != from && meeting != target => Some(meeting),
            (Some(meeting), _) => self.parent(meeting),
            (None, _) => None,
        };
        while let Some(state) = candidate {
            if matches!(self.kind(state), StateKind::Compound | StateKind::Region) {
                return Some(state);
            }
            candidate = self.parent(state);
        }
        None
    }
}
