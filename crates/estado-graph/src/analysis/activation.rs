//! The states that become active when a state is entered (spec 5.2).

use estado_core::{EdgeKind, StateKind};

use crate::tree::Tree;

/// Marks in `active` what entering `state` makes active: the state, its
/// ancestors, every other region of the parallel states among them (entering
/// one region starts them all), and what a default entry activates below the
/// state (the `#[initial]` of a compound state or region, every region of a
/// parallel state).
///
/// `active` only grows, so calling this for several entries gives the union.
///
/// A `goto!(T, history)` is entered as if nothing had been recorded: the
/// substate a history restores was active before, so it is reachable through
/// the edge that entered it then (ADR 0019).
pub(crate) fn activate(tree: &Tree<'_>, state: usize, active: &mut [bool]) {
    let mut current = state;
    loop {
        active[current] = true;
        let Some(parent) = tree.parent(current) else {
            break;
        };
        if tree.kind(parent) == StateKind::Parallel {
            for &region in tree.children(parent) {
                if region != current {
                    active[region] = true;
                    activate_defaults(tree, region, active, active.len());
                }
            }
        }
        current = parent;
    }
    activate_defaults(tree, state, active, active.len());
}

/// `budget` bounds the depth, so a hand-built graph whose `initial` fields
/// form a cycle cannot loop forever.
fn activate_defaults(tree: &Tree<'_>, state: usize, active: &mut [bool], budget: usize) {
    if budget == 0 {
        return;
    }
    match tree.kind(state) {
        StateKind::Parallel => {
            for &region in tree.children(state) {
                active[region] = true;
                activate_defaults(tree, region, active, budget - 1);
            }
        }
        StateKind::Compound | StateKind::Region => {
            if let Some(initial) = tree.graph.states[state].initial {
                active[initial.index()] = true;
                activate_defaults(tree, initial.index(), active, budget - 1);
            }
        }
        StateKind::Atomic | StateKind::Final | StateKind::Choice => {}
    }
}

/// Marks in `active` the states that are certainly active right after the
/// edge `from -> to` of `kind` is taken (spec 5.2): `to` and its ancestors,
/// the states the transition enters below the LCA (the other regions of a
/// parallel state it enters, and the default entry below `to`). A parallel
/// state the transition does not leave keeps its other regions as they were,
/// so nothing is said about them.
pub(crate) fn entered_by(
    tree: &Tree<'_>,
    from: usize,
    to: usize,
    kind: EdgeKind,
    active: &mut [bool],
) {
    let reenters_target =
        to == from || (kind == EdgeKind::External && tree.is_strict_ancestor(to, from));
    let boundary = if reenters_target {
        tree.parent(to)
    } else if tree.is_strict_ancestor(to, from) {
        Some(to)
    } else {
        tree.lowest_common_ancestor(from, to)
    };

    let mut ancestor = Some(to);
    while let Some(state) = ancestor {
        active[state] = true;
        ancestor = tree.parent(state);
    }
    let mut entered = Some(to);
    while let Some(state) = entered {
        if Some(state) == boundary {
            break;
        }
        entered = tree.parent(state);
        if let Some(parent) = entered {
            if Some(parent) != boundary && tree.kind(parent) == StateKind::Parallel {
                for &region in tree.children(parent) {
                    if region != state {
                        active[region] = true;
                        activate_defaults(tree, region, active, active.len());
                    }
                }
            }
        }
    }
    // A local transition to a parallel ancestor says nothing about which of
    // its regions restart.
    let stays_parallel = boundary == Some(to) && tree.kind(to) == StateKind::Parallel;
    if !stays_parallel {
        activate_defaults(tree, to, active, active.len());
    }
}
