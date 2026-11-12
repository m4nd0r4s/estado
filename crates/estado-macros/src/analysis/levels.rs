//! Checks per level of the hierarchy: depth (E0011) and initials (E0010).

use super::MAX_DEPTH;
use crate::model::{Diagnostic, StateTree};

/// A node deeper than [`MAX_DEPTH`] levels is reported once, at the first
/// node past the limit; its descendants are not repeated.
pub fn check_depth(tree: &StateTree, diagnostics: &mut Vec<Diagnostic>) {
    for node in tree.nodes.iter().skip(1) {
        if usize::from(node.depth) == MAX_DEPTH {
            diagnostics.push(
                Diagnostic::new(
                    "E0011",
                    node.name.span(),
                    format!(
                        "`{}` is nested {} levels deep; the maximum is {MAX_DEPTH}",
                        node.name,
                        MAX_DEPTH + 1
                    ),
                )
                .with_help("flatten the hierarchy, or move a branch into a child machine"),
            );
        }
    }
}

/// Every enum of states (root, substates, region) needs exactly one
/// `#[initial]` variant.
///
/// Spec 3.2 allows omitting it when no `goto!` enters the level without an
/// explicit path; that needs the transition targets, so until the edges are
/// resolved (4.2) it is always required. 4.2 relaxes this check.
pub fn check_initials(tree: &StateTree, diagnostics: &mut Vec<Diagnostic>) {
    for node in &tree.nodes {
        let Some(children_enum) = &node.children_enum else {
            continue;
        };
        let initials: Vec<_> = node
            .children
            .iter()
            .map(|&id| tree.node(id))
            .filter(|child| child.is_initial)
            .collect();
        match initials.as_slice() {
            [_single] => {}
            [] => diagnostics.push(
                Diagnostic::new(
                    "E0010",
                    children_enum.span(),
                    format!("`{children_enum}` has no `#[initial]` variant"),
                )
                .with_help("mark the variant entered first with `#[initial]`"),
            ),
            [first, second, ..] => diagnostics.push(
                Diagnostic::new(
                    "E0010",
                    second.name.span(),
                    format!("`{children_enum}` has more than one `#[initial]` variant"),
                )
                .with_help("keep `#[initial]` on exactly one variant")
                .with_note(
                    first.name.span(),
                    format!("`{}` is the first `#[initial]`", first.name),
                ),
            ),
        }
    }
}
