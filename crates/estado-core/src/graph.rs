//! The static graph of a machine (spec 8.1), generated as a `const`.

use crate::ids::StateId;

/// The whole graph of one machine.
///
/// ```
/// use estado_core::{Graph, StateId, StateInfo, StateKind};
///
/// static STATES: [StateInfo; 2] = [
///     StateInfo::new(StateId::new(0), "Active", None, StateKind::Compound, 0),
///     StateInfo::new(StateId::new(1), "Playing", Some(StateId::new(0)), StateKind::Atomic, 1),
/// ];
/// static GRAPH: Graph = Graph { machine: "Player", states: &STATES, edges: &[] };
///
/// let playing = GRAPH.find("Playing").unwrap();
/// assert!(GRAPH.is_ancestor_or_self(StateId::new(0), playing.id));
/// assert!(!GRAPH.is_ancestor_or_self(playing.id, StateId::new(0)));
/// ```
#[derive(Debug)]
pub struct Graph {
    /// Machine name.
    pub machine: &'static str,
    /// Every state, in declaration order; index = [`StateId::index`].
    pub states: &'static [StateInfo],
    /// Every edge declared by a handler.
    pub edges: &'static [EdgeInfo],
}

impl Graph {
    /// Information about `state`.
    ///
    /// # Panics
    ///
    /// If `state` does not belong to this graph.
    pub fn state(&self, state: StateId) -> &StateInfo {
        &self.states[state.index()]
    }

    /// Looks a state up by name.
    pub fn find(&self, name: &str) -> Option<&StateInfo> {
        self.states.iter().find(|info| info.name == name)
    }

    /// Whether `ancestor` is `state` or one of its ancestors.
    pub fn is_ancestor_or_self(&self, ancestor: StateId, state: StateId) -> bool {
        let mut current = Some(state);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.state(id).parent;
        }
        false
    }
}

/// One state of the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateInfo {
    /// Its id.
    pub id: StateId,
    /// Its name, unique in the machine.
    pub name: &'static str,
    /// Its parent, `None` for top-level states.
    pub parent: Option<StateId>,
    /// What kind of state it is.
    pub kind: StateKind,
    /// The initial substate of a compound state or region.
    pub initial: Option<StateId>,
    /// History kept by a compound state.
    pub history: HistoryKind,
    /// Number of `#[after]` timers.
    pub timers: u8,
    /// Number of `#[invoke]` tasks.
    pub invokes: u8,
    /// Nesting depth, 0 for top-level states.
    pub depth: u8,
}

impl StateInfo {
    /// A state without initial, history, timers or invokes.
    pub const fn new(
        id: StateId,
        name: &'static str,
        parent: Option<StateId>,
        kind: StateKind,
        depth: u8,
    ) -> Self {
        Self {
            id,
            name,
            parent,
            kind,
            initial: None,
            history: HistoryKind::None,
            timers: 0,
            invokes: 0,
            depth,
        }
    }

    /// Sets the initial substate.
    pub const fn with_initial(mut self, initial: StateId) -> Self {
        self.initial = Some(initial);
        self
    }

    /// Sets the history kind.
    pub const fn with_history(mut self, history: HistoryKind) -> Self {
        self.history = history;
        self
    }

    /// Sets the number of timers and invokes.
    pub const fn with_tasks(mut self, timers: u8, invokes: u8) -> Self {
        self.timers = timers;
        self.invokes = invokes;
        self
    }
}

/// Kind of a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    /// Leaf without substates or regions.
    Atomic,
    /// Has exactly one `#[substates]` field.
    Compound,
    /// Has two or more `#[region]` fields.
    Parallel,
    /// A region of a parallel state.
    Region,
    /// A `#[terminal]` state (spec 5.10).
    Final,
    /// A `#[choice]` pseudostate.
    ///
    /// Reserved: `#[choice]` is not supported yet (E0090), so the macro
    /// never emits this kind.
    Choice,
}

/// History kept by a compound state (spec 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
    /// No history.
    None,
    /// Last direct substate discriminant.
    Shallow,
    /// Full value of the substate enum.
    DeepKeep,
}

/// One edge: declared by the handler of `from`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeInfo {
    /// State whose handler declares the edge.
    pub from: StateId,
    /// Destination.
    pub to: EdgeTarget,
    /// Kind of edge.
    pub kind: EdgeKind,
    /// Event pattern of the `match` arm, when it could be extracted.
    pub event_hint: Option<&'static str>,
}

/// Destination of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeTarget {
    /// A real state.
    State(StateId),
    /// A `#[choice]` pseudostate.
    ///
    /// Reserved: `#[choice]` is not supported yet (E0090), so the macro
    /// never emits this target.
    Choice(StateId),
    /// `goto_dyn!`: decided at runtime.
    ///
    /// Reserved: `goto_dyn!` is not supported yet (E0090), so the macro
    /// never emits this target.
    Dynamic,
}

/// Kind of edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// Plain `goto!`.
    Normal,
    /// `goto!(T, external)`.
    External,
    /// `goto!(T, history)`.
    History,
    /// `Err` routed to `#[on_error]`.
    ///
    /// Reserved: `#[on_error]` is not supported yet (E0090), so the macro
    /// never emits this kind.
    Error,
    /// Timer event.
    Timer,
    /// `#[always]`.
    Always,
    /// `#[on_done]`.
    Done,
}
