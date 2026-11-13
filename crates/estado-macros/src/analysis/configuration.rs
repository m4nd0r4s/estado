//! Size of the active configuration (E0018).
//!
//! The runtime stores the active configuration in a `StatePath` with room for
//! [`MAX_ACTIVE_STATES`] entries. Every state and every region on the active
//! branches takes one entry; the root enum does not. A machine whose largest
//! possible configuration does not fit is rejected here instead of panicking
//! in `StatePath::push` while a `send` runs.

use super::MAX_ACTIVE_STATES;
use crate::model::{Diagnostic, StateKind, StateTree};

/// For every node, the most entries a configuration can hold while that node
/// is active, the node itself included.
///
/// Children are visited after their parent in pre-order, so walking the nodes
/// backwards sees every child before its parent and needs no recursion.
fn largest_configurations(tree: &StateTree) -> Vec<usize> {
    let mut sizes = vec![0; tree.nodes.len()];
    for node in tree.nodes.iter().rev() {
        let below = node.children.iter().map(|&child| sizes[child]);
        sizes[node.id] = match node.kind {
            // All regions are active at once.
            StateKind::Parallel => 1 + below.sum::<usize>(),
            // One substate is active at a time.
            StateKind::Compound | StateKind::Region => 1 + below.max().unwrap_or(0),
            // The root enum is not an entry of the path.
            StateKind::Root => below.max().unwrap_or(0),
            StateKind::Atomic => 1,
        };
    }
    sizes
}

/// Reports the smallest subtree whose active configuration alone exceeds
/// [`MAX_ACTIVE_STATES`], once: the states above it only inherit the problem.
pub fn check_active_configuration(tree: &StateTree, diagnostics: &mut Vec<Diagnostic>) {
    let sizes = largest_configurations(tree);
    let culprit = tree.nodes.iter().skip(1).find(|node| {
        sizes[node.id] > MAX_ACTIVE_STATES
            && node
                .children
                .iter()
                .all(|&child| sizes[child] <= MAX_ACTIVE_STATES)
    });
    let Some(node) = culprit else {
        return;
    };
    diagnostics.push(
        Diagnostic::new(
            "E0018",
            node.name.span(),
            format!(
                "`{}` can have {} states active at once, regions included; the maximum is {MAX_ACTIVE_STATES}",
                node.name, sizes[node.id]
            ),
        )
        .with_help(
            "use fewer regions, flatten the hierarchy, or move a branch into a child machine",
        ),
    );
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::{format_ident, quote};

    use crate::expand;

    /// A parallel state with `regions` regions of one state each: it holds
    /// `1 + 2 * regions` active entries.
    fn expand_parallel(regions: usize) -> String {
        let fields: Vec<_> = (0..regions)
            .map(|index| format_ident!("region_{index}"))
            .collect();
        let enums: Vec<_> = (0..regions)
            .map(|index| format_ident!("Part{index}"))
            .collect();
        let leaves: Vec<_> = (0..regions)
            .map(|index| format_ident!("Leaf{index}"))
            .collect();
        let module: TokenStream = quote! {
            mod m {
                #[root]
                enum M { #[initial] Wide(Wide) }
                #[state]
                struct Wide { #( #[region] #fields: #enums, )* }
                #( #[state] enum #enums { #[initial] #leaves } )*
                #[handlers]
                impl M {}
            }
        };
        expand::machine(quote! { event = Ev }, module).to_string()
    }

    #[test]
    fn the_limit_is_the_capacity_of_the_runtime_path() {
        assert_eq!(super::MAX_ACTIVE_STATES, estado_core::MAX_ACTIVE_STATES);
    }

    #[test]
    fn sixteen_regions_hold_thirty_three_entries() {
        assert!(expand_parallel(16).contains("`Wide` can have 33 states active at once"));
    }

    #[test]
    fn fifteen_regions_hold_exactly_thirty_one() {
        assert!(!expand_parallel(15).contains("E0018"));
    }
}
