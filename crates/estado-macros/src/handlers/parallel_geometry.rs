//! Transitions whose LCA is a parallel state (spec 5.2).

use proc_macro2::Span;

use crate::model::{Diagnostic, StateId, StateKind, StateTree};

/// Transitions whose LCA is a parallel state: between sibling regions
/// (E0021, spec 5.2), or local ones that would reset every region (not
/// supported yet).
pub fn check_parallel_geometry(
    tree: &StateTree,
    handler: StateId,
    target: StateId,
    lca: StateId,
    macro_span: Span,
) -> Result<(), Diagnostic> {
    if tree.node(lca).kind != StateKind::Parallel {
        return Ok(());
    }
    let parallel = &tree.node(lca).name;
    let regions = (tree.region_of(handler), tree.region_of(target));
    if let (Some(from), Some(to)) = regions {
        if from != to {
            return Err(Diagnostic::new(
                "E0021",
                macro_span,
                format!(
                    "`{}` and `{}` are in sibling regions of `{parallel}`",
                    tree.node(handler).name,
                    tree.node(target).name
                ),
            )
            .with_help(
                "regions never transition each other: raise an event with `cx.raise(..)` that the other region handles",
            ));
        }
    }
    Err(Diagnostic::new(
        "E0090",
        macro_span,
        format!("a local transition between `{parallel}` and its own regions is not supported yet"),
    )
    .with_help(
        "use `goto!(.., external)` to leave and re-enter it, or transition inside one region",
    ))
}
