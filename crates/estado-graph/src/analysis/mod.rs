//! Static analyses of a [`Graph`] (spec 8.3).
//!
//! The analyses read only the graph, so they see what the graph records:
//! edges, kinds and the hierarchy. They do not know guards, entry or exit
//! handlers, or `reenter!()`. ADR 0019 lists the modelling choices:
//! entering a compound state enters its initial chain, entering a parallel
//! state enters every region, and an edge declared by a compound state is
//! available from every descendant.

mod activation;
mod components;
mod cover;
mod model;
mod path;
mod reachable;

use alloc::vec::Vec;

use estado_core::{Graph, StateId};

pub use cover::edge_cover;
pub use path::{Path, shortest_path};
pub use reachable::{dead_ends, reachable};

/// The analyses of spec 8.3 as methods of [`Graph`], like
/// [`GraphExport`](crate::GraphExport) does for the exports.
///
/// The number of entry and exit calls of the costliest transition is
/// `Graph::worst_case_actions`, a `const fn` of `estado-core`.
///
/// ```
/// use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind};
/// use estado_graph::GraphAnalysis;
///
/// const IDLE: StateId = StateId::new(0);
/// const PLAYING: StateId = StateId::new(1);
/// const PAUSED: StateId = StateId::new(2);
/// const fn edge(from: StateId, to: StateId, hint: &'static str) -> EdgeInfo {
///     EdgeInfo { from, to: EdgeTarget::State(to), kind: EdgeKind::Normal, event_hint: Some(hint) }
/// }
///
/// static STATES: [StateInfo; 3] = [
///     StateInfo::new(IDLE, "Idle", None, StateKind::Atomic, 0),
///     StateInfo::new(PLAYING, "Playing", None, StateKind::Atomic, 0),
///     StateInfo::new(PAUSED, "Paused", None, StateKind::Atomic, 0),
/// ];
/// static EDGES: [EdgeInfo; 4] = [
///     edge(IDLE, PLAYING, "Event::Play"),
///     edge(PLAYING, PAUSED, "Event::Pause"),
///     edge(PAUSED, PLAYING, "Event::Play"),
///     edge(PAUSED, IDLE, "Event::Stop"),
/// ];
/// static GRAPH: Graph = Graph {
///     machine: "Player",
///     initial: Some(IDLE),
///     states: &STATES,
///     edges: &EDGES,
/// };
///
/// assert_eq!(GRAPH.reachable().len(), 3);
/// assert!(GRAPH.dead_ends().is_empty());
///
/// let path = GRAPH.shortest_path(IDLE, PAUSED).unwrap();
/// assert_eq!(path.edges(), [0, 1]);
///
/// // One walk takes all four edges: Play, Pause, Play, Pause, Stop.
/// let cover = GRAPH.edge_cover();
/// assert_eq!(cover.len(), 1);
/// assert_eq!(cover[0].edges(), [0, 1, 2, 1, 3]);
///
/// assert_eq!(GRAPH.worst_case_actions(), 2);
/// ```
pub trait GraphAnalysis {
    /// The states that can become active, starting from the initial state
    /// of the machine and following the edges, in declaration order (the
    /// basis of W0001).
    ///
    /// Entering a state also activates its ancestors and what a default
    /// entry activates below it, so the compound state that holds a
    /// reachable state is reachable too. An edge counts when the state that
    /// declares it is reachable, whatever its kind: timer, `#[always]` and
    /// `#[on_done]` edges are followed as if they could fire (an
    /// over-approximation: the graph does not say which completion a state
    /// can reach). A `goto_dyn!` edge from a reachable state makes every
    /// state reachable. Empty for a graph without an initial state.
    fn reachable(&self) -> Vec<StateId>;

    /// The states from which no edge leads out, in declaration order (W0002):
    /// atomic states, not `#[terminal]`, that neither declare an edge nor
    /// have an ancestor that does. Compound and parallel states are not
    /// listed, only their leaves; every state is checked, reachable or not.
    fn dead_ends(&self) -> Vec<StateId>;

    /// The fewest paths from the initial state that, together, take every
    /// edge that can be taken, for model-based tests.
    ///
    /// Every edge whose source state can be activated by following known
    /// edges from the initial state is in some path; an edge whose source
    /// can only be reached through `goto_dyn!` is in none. The paths are
    /// valid walks in the model of ADR 0019 (each edge is declared by a
    /// state the previous one entered), and no other set of walks covers
    /// the edges with fewer paths. Paths can overlap and repeat edges; they
    /// are not the shortest ones. The result is deterministic: paths come
    /// out sorted by their edges. Empty when no edge can be taken.
    fn edge_cover(&self) -> Vec<Path>;

    /// The path with the fewest edges from a configuration where `from` has
    /// just been entered (it, its ancestors and its default entry are
    /// active) to one where `to` is active, `None` when there is none. A
    /// `to` that `from` already activates gives the empty path. Ties go to
    /// the edges declared first.
    ///
    /// # Panics
    ///
    /// If `from` or `to` does not belong to this graph.
    fn shortest_path(&self, from: StateId, to: StateId) -> Option<Path>;
}

impl GraphAnalysis for Graph {
    fn reachable(&self) -> Vec<StateId> {
        reachable(self)
    }

    fn dead_ends(&self) -> Vec<StateId> {
        dead_ends(self)
    }

    fn edge_cover(&self) -> Vec<Path> {
        edge_cover(self)
    }

    fn shortest_path(&self, from: StateId, to: StateId) -> Option<Path> {
        shortest_path(self, from, to)
    }
}
