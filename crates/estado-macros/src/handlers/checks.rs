//! Structural checks of the handler analysis: what the `#[handlers]` blocks
//! may be attached to, the initial chain of the root, nested deep histories
//! and the parallel states this version does not support (E0017, E0020,
//! E0090).

use syn::{ImplItem, ItemImpl, Type};

use super::goto_resolve::missing_data_error;
use super::roles::reject_role_attributes;
use crate::construct::initial_value;
use crate::model::{Diagnostic, Extent, HandlerModel, MachineModel};

/// `#[on(..)]` and its siblings on a constant, a type alias or a macro call
/// of the impl block.
pub fn reject_roles_outside_functions(item: &mut ImplItem, diagnostics: &mut Vec<Diagnostic>) {
    let (attributes, what) = match item {
        ImplItem::Const(item) => (&mut item.attrs, "a constant"),
        ImplItem::Type(item) => (&mut item.attrs, "a type alias"),
        ImplItem::Macro(item) => (&mut item.attrs, "a macro call"),
        _ => return,
    };
    reject_role_attributes(attributes, what, diagnostics);
}

/// A `#[handlers]` block is an inherent `impl` of the root type: the
/// functions in it take `Self` as the machine.
pub fn check_impl_target(
    model: &MachineModel,
    block: &ItemImpl,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let root = &model.tree.root().name;
    let is_root = block.trait_.is_none()
        && matches!(&*block.self_ty, Type::Path(path)
            if path.qself.is_none() && path.path.is_ident(root));
    if !is_root {
        diagnostics.push(
            Diagnostic::new(
                "E0017",
                Extent::of(&block.self_ty),
                format!("`#[handlers]` must be on an inherent `impl {root}`"),
            )
            .with_help(format!(
                "the handlers are functions of the root type `{root}`: write `impl {root} {{ .. }}`"
            )),
        );
    }
    is_root
}

/// `Spec::initial` builds the `#[initial]` chain of the root: every level on
/// it must be buildable without data, or carry `#[initial(with = ..)]` (E0020).
pub fn check_root_initial(model: &MachineModel, diagnostics: &mut Vec<Diagnostic>) {
    let tree = &model.tree;
    if let Err(missing) = initial_value(tree, 0) {
        let node = tree.node(missing.node);
        diagnostics.push(missing_data_error(
            tree,
            &missing,
            &node.name,
            node.name.span().into(),
        ));
    }
}

/// A deep history stores the whole substate value, so a second deep history
/// inside it would need the same value twice (errata 5 covers one level).
pub fn check_history_nesting(model: &MachineModel, diagnostics: &mut Vec<Diagnostic>) {
    use crate::model::HistoryKind;
    let tree = &model.tree;
    for node in tree.nodes.iter().skip(1) {
        if node.history != HistoryKind::DeepKeep {
            continue;
        }
        let outer = node.id;
        let nested = tree.nodes.iter().skip(1).find(|candidate| {
            candidate.id != outer
                && candidate.history == HistoryKind::DeepKeep
                && tree.is_ancestor_or_self(outer, candidate.id)
        });
        if let Some(inner) = nested {
            diagnostics.push(
                Diagnostic::new(
                    "E0090",
                    inner.name.span(),
                    format!(
                        "`{}` keeps deep history inside `{}`, which keeps deep history too",
                        inner.name, node.name
                    ),
                )
                .with_help("nested deep histories are not supported yet; use `#[history]` (shallow) for the inner one"),
            );
        }
    }
}

/// What parallel states do not support yet: a parallel state inside a region
/// of another one (the short-circuit of spec 5.6 item 3 is tracked for one
/// level), and a handler or `#[after]`-style attribute on a region itself.
pub fn check_parallel_support(
    model: &MachineModel,
    analysed: &[HandlerModel],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use crate::model::StateKind;
    let tree = &model.tree;
    for node in tree.nodes.iter().skip(1) {
        if node.kind == StateKind::Parallel && tree.region_of(node.id).is_some() {
            diagnostics.push(
                Diagnostic::new(
                    "E0090",
                    node.name.span(),
                    format!("the parallel state `{}` is inside a region", node.name),
                )
                .with_help("nested parallel states are not supported yet; flatten the regions"),
            );
        }
    }
    for handler in analysed {
        let state = tree.node(handler.state);
        if state.kind == StateKind::Region {
            diagnostics.push(
                Diagnostic::new(
                    "E0090",
                    handler.function.sig.ident.span(),
                    format!(
                        "`{}` is a region: it has no handlers of its own",
                        state.name
                    ),
                )
                .with_help("put the handler on a state of the region, or on the parallel state"),
            );
        }
    }
}
