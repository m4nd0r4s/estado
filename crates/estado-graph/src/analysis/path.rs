//! A path through the graph and `shortest_path`.

use alloc::vec::Vec;

use estado_core::{EdgeInfo, Graph, StateId};

use super::model::EdgeModel;

/// A walk through the edges of a graph.
///
/// It starts with `start` active (the initial state of the machine for the
/// paths of `edge_cover`) and takes [`edges`](Path::edges) one after the
/// other: each one is declared by a state that is active after the one
/// before it. The edges are indices into [`Graph::edges`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    start: StateId,
    edges: Vec<usize>,
}

impl Path {
    pub(crate) fn new(start: StateId, edges: Vec<usize>) -> Self {
        Self { start, edges }
    }

    /// The state that is active before the first edge.
    pub fn start(&self) -> StateId {
        self.start
    }

    /// The edges, as indices into [`Graph::edges`], in the order they are
    /// taken.
    pub fn edges(&self) -> &[usize] {
        &self.edges
    }

    /// The number of edges.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether the path takes no edge.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// The edges of the path, resolved in `graph`.
    ///
    /// # Panics
    ///
    /// If `graph` is not the graph the path was computed for.
    pub fn infos<'graph>(
        &'graph self,
        graph: &'graph Graph,
    ) -> impl Iterator<Item = &'graph EdgeInfo> + 'graph {
        self.edges.iter().map(|&edge| &graph.edges[edge])
    }
}

/// The path with the fewest edges from `from` to `to`. See
/// [`GraphAnalysis::shortest_path`].
///
/// [`GraphAnalysis::shortest_path`]: super::GraphAnalysis::shortest_path
pub fn shortest_path(graph: &Graph, from: StateId, to: StateId) -> Option<Path> {
    let model = EdgeModel::new(graph);
    let start = model.entered(from.index());
    if start[to.index()] {
        return Some(Path::new(from, Vec::new()));
    }
    let sources = model.enabled_by(&start);
    let walk = model.search(&sources, |edge| model.active_after(edge, to.index()))?;
    Some(Path::new(from, walk))
}
