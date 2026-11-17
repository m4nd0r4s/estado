//! Worst-case number of entry and exit calls of one transition (spec 8.3).
//!
//! Everything here is `const fn` and allocation free, so a machine can
//! evaluate its bound at compile time (`Motor::WORST_CASE_ACTIONS`). The
//! price is that the helpers rescan the state table instead of memoising;
//! that is fine for the machines this is meant for and for any machine of a
//! few dozen states. A table of hundreds of states may exceed the compiler's
//! `long_running_const_eval` budget when the bound is evaluated in a
//! constant; evaluating it at run time has no such limit.
//!
//! The model follows spec 5.2 and 5.3 (ADR 0019):
//!
//! - every state that exits or enters counts once, whether or not it
//!   declares an `#[entry]` or `#[exit]` (the graph does not say), so the
//!   result is an upper bound;
//! - regions and `#[choice]` pseudostates have no handlers and count zero;
//! - the active configuration below the source is the worst one: the
//!   deepest chain of a compound state, every region of a parallel state;
//! - entering a compound state enters its `#[initial]` chain, entering a
//!   parallel state enters every region; a `goto!(T, history)` enters
//!   whatever the history may hold, so it counts the largest configuration
//!   below `T`;
//! - a target that is only known at run time (`goto_dyn!`, `#[choice]`)
//!   counts as the costliest real target.

use crate::graph::{EdgeInfo, EdgeKind, EdgeTarget, Graph, StateKind};
use crate::ids::StateId;

/// A state kind's number of handler calls per exit or entry.
const fn weight(kind: StateKind) -> usize {
    match kind {
        StateKind::Region | StateKind::Choice => 0,
        StateKind::Atomic | StateKind::Compound | StateKind::Parallel | StateKind::Final => 1,
    }
}

const fn max(left: usize, right: usize) -> usize {
    if left > right { left } else { right }
}

impl Graph {
    /// The largest number of `#[entry]` and `#[exit]` calls one transition
    /// of this machine can make: the maximum of [`Graph::edge_actions`] over
    /// the edges of the graph, `0` for a graph without edges.
    ///
    /// A transition is one microstep: the exits, then the entries of spec
    /// 5.3. Chained microsteps (`#[always]`, `#[on_done]`, raised events)
    /// are other transitions and are not added up. The count is an upper
    /// bound, see the [module documentation](self) for the model.
    ///
    /// This is a `const fn`: `#[machine]` evaluates it, plus the
    /// `reenter!()` sites the graph does not list, into
    /// `Root::WORST_CASE_ACTIONS`.
    ///
    /// ```
    /// use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind};
    ///
    /// const IDLE: StateId = StateId::new(0);
    /// const ACTIVE: StateId = StateId::new(1);
    /// const PLAYING: StateId = StateId::new(2);
    /// static STATES: [StateInfo; 3] = [
    ///     StateInfo::new(IDLE, "Idle", None, StateKind::Atomic, 0),
    ///     StateInfo::new(ACTIVE, "Active", None, StateKind::Compound, 0).with_initial(PLAYING),
    ///     StateInfo::new(PLAYING, "Playing", Some(ACTIVE), StateKind::Atomic, 1),
    /// ];
    /// static EDGES: [EdgeInfo; 1] = [EdgeInfo {
    ///     from: IDLE,
    ///     to: EdgeTarget::State(ACTIVE),
    ///     kind: EdgeKind::Normal,
    ///     event_hint: Some("Event::Play"),
    /// }];
    /// static GRAPH: Graph = Graph {
    ///     machine: "Player",
    ///     initial: Some(IDLE),
    ///     states: &STATES,
    ///     edges: &EDGES,
    /// };
    ///
    /// // Idle exits; Active and Playing enter.
    /// const WORST: usize = GRAPH.worst_case_actions();
    /// assert_eq!(WORST, 3);
    /// ```
    pub const fn worst_case_actions(&self) -> usize {
        let mut worst = 0;
        let mut index = 0;
        while index < self.edges.len() {
            worst = max(worst, self.edge_actions(&self.edges[index]));
            index += 1;
        }
        worst
    }

    /// The largest number of entry and exit calls `edge` can make: the
    /// exits of the deepest configuration below its source, plus the entries
    /// down to its target and the initial states below it.
    ///
    /// # Panics
    ///
    /// If the edge names a state that does not belong to this graph.
    pub const fn edge_actions(&self, edge: &EdgeInfo) -> usize {
        match edge.to {
            EdgeTarget::State(target) => self.transition_actions(edge.from, target, edge.kind),
            EdgeTarget::Choice(_) | EdgeTarget::Dynamic => {
                // The destination is decided at run time: take the costliest.
                let mut worst = 0;
                let mut index = 0;
                while index < self.states.len() {
                    let kind = self.states[index].kind;
                    if weight(kind) > 0 {
                        let target = self.states[index].id;
                        worst = max(
                            worst,
                            self.transition_actions(edge.from, target, EdgeKind::Normal),
                        );
                        worst = max(
                            worst,
                            self.transition_actions(edge.from, target, EdgeKind::External),
                        );
                    }
                    index += 1;
                }
                worst
            }
        }
    }

    /// The entry and exit calls of `reenter!()` in a handler of `state`:
    /// the same as `goto!(state)`, an external transition to itself.
    ///
    /// `reenter!()` is not an edge of the graph, so [`Graph::worst_case_actions`]
    /// does not see it; the macro adds it for the handlers that use it.
    pub const fn reenter_actions(&self, state: StateId) -> usize {
        self.transition_actions(state, state, EdgeKind::External)
    }

    /// Exits plus entries of a transition decided by `source` towards
    /// `target` (spec 5.2).
    const fn transition_actions(&self, source: StateId, target: StateId, kind: EdgeKind) -> usize {
        let source = source.index();
        let target = target.index();
        let history = matches!(kind, EdgeKind::History);
        if source == target {
            return self.exit_actions(source, source) + self.entry_actions(source, target, history);
        }
        if self.is_strict_ancestor(target, source) {
            if matches!(kind, EdgeKind::External) {
                // Exits `target` itself and enters it again.
                return self.exit_actions(target, source)
                    + self.entry_actions(target, target, history);
            }
            // Local: `target` stays; what is below it is left and entered.
            let exits = if matches!(self.states[target].kind, StateKind::Parallel) {
                self.below_actions(target, true)
            } else {
                self.exit_actions(self.child_towards(target, source), source)
            };
            return exits + self.below_actions(target, history);
        }
        if self.is_strict_ancestor(source, target) {
            // Local: `source` stays; everything below it is left.
            let exits = self.below_actions(source, true);
            return exits + self.entry_actions(self.child_towards(source, target), target, history);
        }
        let meeting = self.lowest_common_ancestor(source, target);
        let exit_root = self.top_below(meeting, source);
        let entry_root = self.top_below(meeting, target);
        self.exit_actions(exit_root, source) + self.entry_actions(entry_root, target, history)
    }

    /// States left when `source`, below `top` (or equal to it), is active
    /// and `top` exits: `top` down to the deepest configuration below
    /// `source`, plus every other region of a parallel state on the way.
    const fn exit_actions(&self, top: usize, source: usize) -> usize {
        let mut total = self.max_active(source);
        let mut node = source;
        while node != top {
            let parent = self.parent_of(node);
            total += weight(self.states[parent].kind);
            if matches!(self.states[parent].kind, StateKind::Parallel) {
                total += self.other_children(parent, node, true);
            }
            node = parent;
        }
        total
    }

    /// States entered from `top` down to `target` (both included), then
    /// below `target`, plus every other region of a parallel state on the way.
    const fn entry_actions(&self, top: usize, target: usize, history: bool) -> usize {
        let mut total = weight(self.states[target].kind) + self.below_actions(target, history);
        let mut node = target;
        while node != top {
            let parent = self.parent_of(node);
            total += weight(self.states[parent].kind);
            if matches!(self.states[parent].kind, StateKind::Parallel) {
                total += self.other_children(parent, node, false);
            }
            node = parent;
        }
        total
    }

    /// The states strictly below `state` that are active in the worst
    /// configuration (`worst`) or that its default entry activates.
    const fn below_actions(&self, state: usize, worst: bool) -> usize {
        if worst {
            self.max_active(state) - weight(self.states[state].kind)
        } else {
            self.default_entry(state) - weight(self.states[state].kind)
        }
    }

    /// Handler calls of the worst configuration under `state`, `state`
    /// included: the deepest chain of a compound state or region, every
    /// region of a parallel state.
    const fn max_active(&self, state: usize) -> usize {
        let own = weight(self.states[state].kind);
        match self.states[state].kind {
            StateKind::Parallel => own + self.children_sum(state, true),
            StateKind::Compound | StateKind::Region => own + self.children_max(state),
            StateKind::Atomic | StateKind::Final | StateKind::Choice => own,
        }
    }

    /// Handler calls of entering `state` by default, `state` included: its
    /// initial chain, every region of a parallel state.
    const fn default_entry(&self, state: usize) -> usize {
        let own = weight(self.states[state].kind);
        match self.states[state].kind {
            StateKind::Parallel => own + self.children_sum(state, false),
            StateKind::Compound | StateKind::Region => match self.states[state].initial {
                Some(initial) => own + self.default_entry(initial.index()),
                None => own,
            },
            StateKind::Atomic | StateKind::Final | StateKind::Choice => own,
        }
    }

    const fn children_max(&self, state: usize) -> usize {
        let mut worst = 0;
        let mut index = 0;
        while index < self.states.len() {
            if self.is_child_of(index, state) {
                worst = max(worst, self.max_active(index));
            }
            index += 1;
        }
        worst
    }

    /// The sum over the children of `state` (the regions of a parallel
    /// state).
    const fn children_sum(&self, state: usize, worst: bool) -> usize {
        self.other_children(state, usize::MAX, worst)
    }

    /// The sum over the children of `state` except `skip`.
    const fn other_children(&self, state: usize, skip: usize, worst: bool) -> usize {
        let mut total = 0;
        let mut index = 0;
        while index < self.states.len() {
            if index != skip && self.is_child_of(index, state) {
                total += if worst {
                    self.max_active(index)
                } else {
                    self.default_entry(index)
                };
            }
            index += 1;
        }
        total
    }

    const fn is_child_of(&self, child: usize, parent: usize) -> bool {
        match self.states[child].parent {
            Some(candidate) => candidate.index() == parent,
            None => false,
        }
    }

    const fn parent_of(&self, state: usize) -> usize {
        match self.states[state].parent {
            Some(parent) => parent.index(),
            None => panic!("the state has no parent"),
        }
    }

    /// Whether `ancestor` is above `state`, not equal to it.
    const fn is_strict_ancestor(&self, ancestor: usize, state: usize) -> bool {
        let mut current = self.states[state].parent;
        while let Some(parent) = current {
            if parent.index() == ancestor {
                return true;
            }
            current = self.states[parent.index()].parent;
        }
        false
    }

    /// The child of `ancestor` that is `descendant` or above it.
    const fn child_towards(&self, ancestor: usize, descendant: usize) -> usize {
        let mut node = descendant;
        while self.parent_of(node) != ancestor {
            node = self.parent_of(node);
        }
        node
    }

    /// The lowest state above or equal to both, `None` for the root.
    const fn lowest_common_ancestor(&self, left: usize, right: usize) -> Option<usize> {
        let mut candidate = Some(left);
        while let Some(state) = candidate {
            if state == right || self.is_strict_ancestor(state, right) {
                return Some(state);
            }
            candidate = match self.states[state].parent {
                Some(parent) => Some(parent.index()),
                None => None,
            };
        }
        None
    }

    /// The child of `meeting` towards `state`, or the top-level state above
    /// it when `meeting` is the root.
    const fn top_below(&self, meeting: Option<usize>, state: usize) -> usize {
        match meeting {
            Some(ancestor) => self.child_towards(ancestor, state),
            None => {
                let mut node = state;
                while let Some(parent) = self.states[node].parent {
                    node = parent.index();
                }
                node
            }
        }
    }
}
