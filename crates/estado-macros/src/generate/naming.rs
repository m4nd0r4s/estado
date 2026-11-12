//! Names of generated bindings, functions and types (spec 6.5: descriptive,
//! no hashes).

use proc_macro2::Span;
use syn::Ident;

use crate::model::{StateId, StateTree};
use crate::naming::snake_case_name;

fn ident(text: &str, span: Span) -> Ident {
    Ident::new(text, span)
}

/// `active_fields`: the binding of the `*Fields` view of a compound state.
pub fn fields_binding(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("{}_fields", snake_case_name(name)), name.span())
}

/// `active_substates`: the binding of the `&mut` children enum of a compound
/// state (or of the root).
pub fn children_binding(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("{}_substates", snake_case_name(name)), name.span())
}

/// `active_state`: the binding of the payload of a variant.
pub fn payload_binding(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("{}_state", snake_case_name(name)), name.span())
}

/// `failed_reason`: the binding of one field of a variant.
pub fn field_binding(tree: &StateTree, node: StateId, field: &str) -> Ident {
    let name = &tree.node(node).name;
    ident(
        &format!(
            "{}_{}",
            snake_case_name(name),
            field.trim_start_matches("r#")
        ),
        name.span(),
    )
}

/// `exit_active_sub`: exits the active child of the children enum of `owner`.
pub fn exit_function(tree: &StateTree, owner: StateId) -> Ident {
    let enum_name = tree.node(owner).children_enum.as_ref().expect("an enum");
    ident(
        &format!("exit_{}", snake_case_name(enum_name)),
        enum_name.span(),
    )
}

/// `enter_active_sub`.
pub fn enter_function(tree: &StateTree, owner: StateId) -> Ident {
    let enum_name = tree.node(owner).children_enum.as_ref().expect("an enum");
    ident(
        &format!("enter_{}", snake_case_name(enum_name)),
        enum_name.span(),
    )
}

/// `bubble_to_active`: runs the handler of a compound ancestor.
pub fn bubble_function(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("bubble_to_{}", snake_case_name(name)), name.span())
}

/// `reenter_playing`.
pub fn reenter_function(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("reenter_{}", snake_case_name(name)), name.span())
}

/// `ActiveFields`.
pub fn fields_type(tree: &StateTree, node: StateId) -> Ident {
    let name = tree
        .node(node)
        .composite_struct
        .as_ref()
        .expect("a composite");
    ident(&format!("{name}Fields"), name.span())
}

/// `PlayingMut`.
pub fn view_type(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("{name}Mut"), name.span())
}

/// `FailedExiting`.
pub fn exiting_struct(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("{name}Exiting"), name.span())
}
