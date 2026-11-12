//! Names of generated bindings, functions and types (spec 6.5: descriptive,
//! no hashes).

use proc_macro2::Span;
use syn::Ident;

use crate::model::{StateId, StateTree};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::context::Generation;
use crate::naming::{exiting_name, plain_name, snake_case_name, state_constant_name};

fn ident(text: &str, span: Span) -> Ident {
    Ident::new(text, span)
}

/// A local variable or parameter of generated code that user expressions
/// pasted next to it (the closures of `#[after]` and `#[invoke]`) must not
/// see: with a `mixed_site` span it is hygienic, so a function of the user's
/// called `epoch` or `key` still resolves to the function (ADR 0013 item 15).
pub fn hygienic_local(text: &str) -> Ident {
    Ident::new(text, Span::mixed_site())
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

/// `exit_active_sub`, `start_active_sub`, `timer_event_active_sub`, ..: the
/// function of one kind over the children enum of `owner`.
pub fn children_function(prefix: &str, tree: &StateTree, owner: StateId) -> Ident {
    let enum_name = tree.node(owner).children_enum.as_ref().expect("an enum");
    ident(
        &format!("{prefix}_{}", snake_case_name(enum_name)),
        enum_name.span(),
    )
}

/// `exit_active_sub`: exits the active child of the children enum of `owner`.
pub fn exit_function(tree: &StateTree, owner: StateId) -> Ident {
    children_function("exit", tree, owner)
}

/// `enter_active_sub`.
pub fn enter_function(tree: &StateTree, owner: StateId) -> Ident {
    children_function("enter", tree, owner)
}

/// `Root::PLAYING`: the id constant of a state.
pub fn state_constant(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let root = generation.root;
    let constant = format_ident!("{}", state_constant_name(&generation.tree.node(node).name));
    quote! { #root::#constant }
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
    ident(&format!("{}Fields", plain_name(name)), name.span())
}

/// `PlayingMut`.
pub fn view_type(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("{}Mut", plain_name(name)), name.span())
}

/// `FailedExiting`.
pub fn exiting_struct(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    exiting_name(name)
}

/// `BackoffRef`: the shared view of a struct-like or tuple variant that
/// timer and invoke closures receive (spec 7.1).
pub fn ref_view_type(tree: &StateTree, node: StateId) -> Ident {
    let name = &tree.node(node).name;
    ident(&format!("{}Ref", plain_name(name)), name.span())
}
