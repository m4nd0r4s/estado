//! Checks per level of the hierarchy: depth (E0011) and initials (E0010).

use super::MAX_DEPTH;
use crate::model::{Diagnostic, HistoryKind, StateTree};

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

/// Counts the generated code keeps in a `u8`: the index of a timer or an
/// invoke (the graph stores their number in a `u8` too) and the discriminant
/// of a shallow history. More would silently collide, so they are rejected
/// (E0090) instead of wrapping.
pub fn check_counts(tree: &StateTree, diagnostics: &mut Vec<Diagnostic>) {
    let limit = usize::from(u8::MAX);
    for node in tree.nodes.iter().skip(1) {
        let too_many = |what: &str, count: usize, maximum: usize| {
            Diagnostic::new(
                "E0090",
                node.name.span(),
                format!(
                    "`{}` has {count} {what}; the maximum is {maximum}",
                    node.name
                ),
            )
            .with_help("split the state, or move the extra ones to a child state")
        };
        if node.timers.len() > limit {
            diagnostics.push(too_many("`#[after]` timers", node.timers.len(), limit));
        }
        if node.invokes.len() > limit {
            diagnostics.push(too_many("`#[invoke]` tasks", node.invokes.len(), limit));
        }
        if node.history == HistoryKind::Shallow && node.children.len() > limit + 1 {
            diagnostics.push(too_many(
                "substates with shallow history",
                node.children.len(),
                limit + 1,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::quote;

    use crate::expand;

    fn expand_with(timers: usize, substates: usize) -> String {
        let after = (0..timers).map(|_| quote! { #[after(Duration::ZERO => Ev::Tick)] });
        let names: Vec<_> = (0..substates)
            .map(|index| quote::format_ident!("Leaf{index}"))
            .collect();
        let module: TokenStream = quote! {
            mod m {
                #[root]
                enum M {
                    #[initial]
                    #(#after)*
                    Idle,
                    Group(Group),
                }
                #[state]
                #[history]
                struct Group { #[substates] sub: GroupSub }
                #[state]
                enum GroupSub { #[initial] First, #(#names),* }
                #[handlers]
                impl M {}
            }
        };
        expand::machine(quote! { event = Ev }, module).to_string()
    }

    #[test]
    fn more_timers_than_a_u8_indexes_is_e0090() {
        assert!(expand_with(256, 1).contains("has 256 `#[after]` timers"));
        assert!(!expand_with(255, 1).contains("E0090"));
    }

    #[test]
    fn more_substates_than_a_u8_discriminant_is_e0090() {
        assert!(expand_with(1, 256).contains("substates with shallow history"));
        assert!(!expand_with(1, 255).contains("E0090"));
    }
}
