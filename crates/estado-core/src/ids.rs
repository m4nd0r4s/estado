//! State identifiers and active-configuration paths.

use core::fmt;

use crate::graph::{Graph, StateKind};

/// Identifier of a state inside one machine: its index in [`Graph::states`].
///
/// ```
/// use estado_core::StateId;
/// assert_eq!(StateId::new(3).index(), 3);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct StateId(u16);

impl StateId {
    /// Builds an id from a graph index.
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    /// The graph index of this state.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Maximum number of simultaneously active states (all levels, all regions).
pub const MAX_ACTIVE_STATES: usize = 32;

/// The active configuration, without allocating: the active states in
/// pre-order (parent before children, regions in declaration order).
///
/// `Display` renders it as `Connected/link:Open,keepalive:Pinging`.
///
/// ```
/// use estado_core::{Graph, StateId, StateInfo, StateKind, StatePath};
///
/// static STATES: [StateInfo; 2] = [
///     StateInfo::new(StateId::new(0), "Active", None, StateKind::Compound, 0),
///     StateInfo::new(StateId::new(1), "Playing", Some(StateId::new(0)), StateKind::Atomic, 1),
/// ];
/// static GRAPH: Graph = Graph { machine: "Player", states: &STATES, edges: &[] };
///
/// let mut path = StatePath::new(&GRAPH);
/// path.push(StateId::new(0));
/// path.push(StateId::new(1));
/// assert_eq!(path.to_string(), "Active/Playing");
/// assert!(path.contains(StateId::new(1)));
/// ```
#[derive(Clone, Copy)]
pub struct StatePath {
    graph: &'static Graph,
    states: [StateId; MAX_ACTIVE_STATES],
    length: u8,
}

impl StatePath {
    /// An empty path over `graph`.
    pub const fn new(graph: &'static Graph) -> Self {
        Self {
            graph,
            states: [StateId(0); MAX_ACTIVE_STATES],
            length: 0,
        }
    }

    /// Appends the next active state in pre-order.
    ///
    /// # Panics
    ///
    /// If the path already holds [`MAX_ACTIVE_STATES`] states. A machine built
    /// with `#[machine]` never gets there: the macro rejects, at compile time
    /// (`E0018`), a machine whose active configuration could outgrow the path.
    /// A graph built by hand has no such check.
    pub fn push(&mut self, state: StateId) {
        assert!(
            usize::from(self.length) < MAX_ACTIVE_STATES,
            "more than MAX_ACTIVE_STATES ({MAX_ACTIVE_STATES}) states are active at once"
        );
        self.states[usize::from(self.length)] = state;
        self.length += 1;
    }

    /// The active states, in pre-order.
    pub fn states(&self) -> &[StateId] {
        &self.states[..usize::from(self.length)]
    }

    /// Whether `state` is active.
    pub fn contains(&self, state: StateId) -> bool {
        self.states().contains(&state)
    }

    /// The deepest active state of the first branch (the leaf when there are
    /// no regions).
    pub fn leaf(&self) -> Option<StateId> {
        self.states().last().copied()
    }
}

impl PartialEq for StatePath {
    fn eq(&self, other: &Self) -> bool {
        self.states() == other.states()
    }
}

impl Eq for StatePath {}

impl fmt::Debug for StatePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "StatePath({self})")
    }
}

impl fmt::Display for StatePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut previous_kind = None;
        for state in self.states() {
            let info = self.graph.state(*state);
            let separator = match (info.kind, previous_kind) {
                (_, None) => "",
                (StateKind::Region, Some(StateKind::Parallel)) => "/",
                (StateKind::Region, _) => ",",
                (_, Some(StateKind::Region)) => "",
                _ => "/",
            };
            formatter.write_str(separator)?;
            formatter.write_str(info.name)?;
            if info.kind == StateKind::Region {
                formatter.write_str(":")?;
            }
            previous_kind = Some(info.kind);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::StateInfo;

    static STATES: [StateInfo; 1] = [StateInfo::new(
        StateId(0),
        "Only",
        None,
        StateKind::Atomic,
        0,
    )];
    static GRAPH: Graph = Graph {
        machine: "Hand",
        states: &STATES,
        edges: &[],
    };

    #[test]
    fn a_path_holds_exactly_the_maximum() {
        let mut path = StatePath::new(&GRAPH);
        for _ in 0..MAX_ACTIVE_STATES {
            path.push(StateId(0));
        }
        assert_eq!(path.states().len(), MAX_ACTIVE_STATES);
    }

    #[test]
    #[should_panic(expected = "more than MAX_ACTIVE_STATES (32) states are active at once")]
    fn pushing_past_the_maximum_panics_with_a_clear_message() {
        let mut path = StatePath::new(&GRAPH);
        for _ in 0..=MAX_ACTIVE_STATES {
            path.push(StateId(0));
        }
    }
}
