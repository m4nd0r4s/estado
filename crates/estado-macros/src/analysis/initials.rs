//! The two forms of `#[initial(with = f)]` (errata 15): `f()` returns the
//! value of the level, or `f` takes a `&mut Cx<..>` and the `*Fields` of the
//! parent state, in any order, and runs when the level is entered.
//!
//! The macro picks the form from the signature of `f`, read from the machine
//! module: a free function (`with = f`) or a function of the root's
//! `#[handlers]` impl (`with = Self::f` or `with = Root::f`). A path to
//! anything else is called without arguments, the form of earlier versions.

use syn::{FnArg, ImplItem, Item, Path, Signature, Type};

use crate::model::{Diagnostic, Extent, InitialArgument, MachineModel, StateId};
use crate::naming::plain_name;

/// Fills `initial_arguments` of every state with `#[initial(with = ..)]`.
pub fn resolve_initial_arguments(model: &mut MachineModel) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut resolved: Vec<(StateId, Vec<InitialArgument>)> = Vec::new();
    for node in &model.tree.nodes {
        let Some(path) = &node.initial_with else {
            continue;
        };
        let Some(signature) = find_signature(model, path) else {
            continue;
        };
        let arguments = classify(model, node.id, path, signature, &mut diagnostics);
        resolved.push((node.id, arguments));
    }
    for (id, arguments) in resolved {
        model.tree.nodes[id].initial_arguments = arguments;
    }
    diagnostics
}

fn find_signature<'model>(model: &'model MachineModel, path: &Path) -> Option<&'model Signature> {
    let segments: Vec<_> = path.segments.iter().collect();
    match segments.as_slice() {
        [name] => model.passthrough.iter().find_map(|item| match item {
            Item::Fn(function) if function.sig.ident == name.ident => Some(&function.sig),
            _ => None,
        }),
        [owner, name] if owner.ident == "Self" || owner.ident == model.tree.root().name => model
            .handlers
            .iter()
            .flat_map(|block| &block.items)
            .find_map(|item| match item {
                ImplItem::Fn(function) if function.sig.ident == name.ident => Some(&function.sig),
                _ => None,
            }),
        _ => None,
    }
}

fn classify(
    model: &MachineModel,
    node: StateId,
    path: &Path,
    signature: &Signature,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<InitialArgument> {
    let tree = &model.tree;
    let parent_fields = tree.fields_owner(node).map(|parent| {
        let name = tree
            .node(parent)
            .composite_struct
            .as_ref()
            .expect("a composite");
        quote::format_ident!("{}Fields", plain_name(name))
    });
    let mut arguments = Vec::new();
    for argument in &signature.inputs {
        let FnArg::Typed(typed) = argument else {
            diagnostics.push(malformed(
                Extent::of(argument),
                "an `#[initial(with = ..)]` function is not a method",
            ));
            continue;
        };
        let kind = if is_context(&typed.ty) {
            Some(InitialArgument::Context)
        } else if parent_fields
            .as_ref()
            .is_some_and(|fields| last_segment_is(&typed.ty, fields))
        {
            Some(InitialArgument::ParentFields)
        } else {
            None
        };
        match kind {
            Some(kind) if arguments.contains(&kind) => diagnostics.push(malformed(
                Extent::of(&typed.ty),
                "this parameter is given twice",
            )),
            Some(kind) => arguments.push(kind),
            None => diagnostics.push(unknown_parameter(path, &typed.ty, parent_fields.as_ref())),
        }
    }
    arguments
}

fn malformed(span: impl Into<Extent>, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E0017", span, message)
}

fn unknown_parameter(path: &Path, ty: &Type, parent_fields: Option<&syn::Ident>) -> Diagnostic {
    let accepted = match parent_fields {
        Some(fields) => format!("`&mut Cx<..>` and `{fields}<'_>`"),
        None => "`&mut Cx<..>` (the root has no parent state, so no fields)".to_owned(),
    };
    malformed(
        Extent::of(ty),
        format!(
            "`{}` takes a parameter that `#[initial(with = ..)]` cannot give",
            path_text(path)
        ),
    )
    .with_help(format!(
        "an initializer takes {accepted}, in any order, or nothing"
    ))
}

fn path_text(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// `&mut Cx<..>`.
fn is_context(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => {
            reference.mutability.is_some()
                && matches!(&*reference.elem, Type::Path(path)
                    if path.path.segments.last().is_some_and(|segment| segment.ident == "Cx"))
        }
        _ => false,
    }
}

fn last_segment_is(ty: &Type, name: &syn::Ident) -> bool {
    matches!(ty, Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == *name))
}
