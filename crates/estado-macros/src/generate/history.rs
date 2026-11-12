//! History (spec 3.4, 5.3 step 3, errata 5, ADR 0006).
//!
//! Shallow history is recorded in the exit functions, by reference: the
//! discriminant of the substate. Deep history (`keep`) moves the substate
//! value into the runtime, so it is recorded where the exiting value is owned:
//! in `retire_<enum>` (simple-form edges, which overwrite the old value) and in
//! the constructor conversion of `Exiting![X]`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use super::context::Generation;
use super::naming::{children_binding, field_binding, payload_binding};
use super::runtime::history_field;
use super::walk::{Binding, variant_pattern};
use crate::model::{HistoryKind, Payload, StateId, StateKind};
use crate::naming::snake_case_name;

/// `playing_sub_discriminant`.
pub fn discriminant_function(enum_name: &Ident) -> Ident {
    format_ident!("{}_discriminant", snake_case_name(enum_name))
}

/// `playing_sub_from_discriminant`.
pub fn from_discriminant_function(enum_name: &Ident) -> Ident {
    format_ident!("{}_from_discriminant", snake_case_name(enum_name))
}

/// `retire_active_sub`.
pub fn retire_function(enum_name: &Ident) -> Ident {
    format_ident!("retire_{}", snake_case_name(enum_name))
}

/// Whether some compound in the subtree below `owner` (a children owner)
/// has deep history.
pub fn has_deep_below(generation: &Generation<'_>, owner: StateId) -> bool {
    let tree = generation.tree;
    tree.node(owner).children.iter().any(|&child| {
        let node = tree.node(child);
        node.history == HistoryKind::DeepKeep
            || (matches!(
                node.kind,
                StateKind::Compound | StateKind::Parallel | StateKind::Region
            ) && has_deep_below(generation, child))
    })
}

/// Discriminant functions of the compounds with shallow history, and the
/// inverse when every substate is a unit variant (so it can be rebuilt).
pub fn discriminant_functions(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let mut functions = TokenStream::new();
    for node in tree.nodes.iter().skip(1) {
        if node.history != HistoryKind::Shallow {
            continue;
        }
        let enum_name = node.children_enum.as_ref().expect("a compound");
        let discriminant = discriminant_function(enum_name);
        let arms = node.children.iter().enumerate().map(|(index, &child)| {
            let pattern = variant_pattern(generation, child, Binding::Nothing);
            let index = u8::try_from(index).unwrap_or(u8::MAX);
            quote! { #pattern => #index, }
        });
        let documentation =
            format!("The index of the active variant of `{enum_name}` (shallow history).");
        functions.extend(quote! {
            #[doc = #documentation]
            pub fn #discriminant(value: &#enum_name) -> u8 {
                match value { #(#arms)* }
            }
        });
        if shallow_restorable(generation, node.id) {
            let restore = from_discriminant_function(enum_name);
            let arms = node.children.iter().enumerate().map(|(index, &child)| {
                let state = tree.node(child);
                let variant = &state.name;
                let index = u8::try_from(index).unwrap_or(u8::MAX);
                quote! { #index => #enum_name::#variant, }
            });
            let last = tree
                .node(*node.children.last().expect("children"))
                .name
                .clone();
            let documentation =
                format!("The variant of `{enum_name}` at an index recorded by shallow history.");
            functions.extend(quote! {
                #[doc = #documentation]
                pub fn #restore(discriminant: u8) -> #enum_name {
                    match discriminant { #(#arms)* _ => #enum_name::#last }
                }
            });
        }
    }
    functions
}

/// Whether the substates of a shallow-history compound are all unit
/// variants, the only shape rebuilt from a discriminant alone.
pub fn shallow_restorable(generation: &Generation<'_>, node: StateId) -> bool {
    let tree = generation.tree;
    tree.node(node)
        .children
        .iter()
        .all(|&child| matches!(tree.node(child).payload, Payload::Unit))
}

/// The statement an exit function runs after exiting a compound with shallow
/// history: record the discriminant of its substate.
pub fn record_shallow(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(node);
    if state.history != HistoryKind::Shallow {
        return TokenStream::new();
    }
    let enum_name = state.children_enum.as_ref().expect("a compound");
    let discriminant = discriminant_function(enum_name);
    let field = history_field(generation, node);
    let substates = children_binding(tree, node);
    quote! { cx.runtime_mut().#field = ::core::option::Option::Some(#discriminant(#substates)); }
}

/// The statements that move the substate of a deep-history compound out of
/// its owned payload into the runtime. `state` is the owned struct; the
/// remaining shared fields are bound under their [`field_binding`] names.
pub fn record_deep(generation: &Generation<'_>, node: StateId, state: &TokenStream) -> TokenStream {
    let tree = generation.tree;
    let node_info = tree.node(node);
    let struct_name = node_info.composite_struct.as_ref().expect("a compound");
    let children_field = node_info.children_field.as_ref().expect("substates");
    let shared = tree.shared_fields(node);
    let pairs = shared
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .map(|name| {
            let binding = field_binding(tree, node, &name.to_string());
            quote! { #name: #binding }
        });
    let field = history_field(generation, node);
    quote! {
        let #struct_name { #(#pairs,)* #children_field: recorded } = #state;
        cx.runtime_mut().#field = ::core::option::Option::Some(recorded);
    }
}

/// `retire_<enum>(old, cx)` for every children owner whose subtree has deep
/// history: consumes the value that leaves and records the deep histories.
pub fn retire_functions(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let mut functions = TokenStream::new();
    for node in &tree.nodes {
        if !tree.owns_children(node.id) || !has_deep_below(generation, node.id) {
            continue;
        }
        let enum_name = node.children_enum.as_ref().expect("children enum");
        let name = retire_function(enum_name);
        let arms = node.children.iter().map(|&child| {
            let state = tree.node(child);
            let payload = payload_binding(tree, child);
            let pattern = variant_pattern(generation, child, Binding::Everything);
            match (state.kind, state.history) {
                (StateKind::Compound, HistoryKind::DeepKeep) => {
                    let record = record_deep(generation, child, &quote! { #payload });
                    quote! { #pattern => { #record } }
                }
                (StateKind::Compound, _) if has_deep_below(generation, child) => {
                    let field = state.children_field.as_ref().expect("substates");
                    let inner = retire_function(state.children_enum.as_ref().expect("enum"));
                    quote! { #pattern => #inner(#payload.#field, cx), }
                }
                (StateKind::Parallel, _) if has_deep_below(generation, child) => {
                    let retire = state
                        .children
                        .iter()
                        .filter(|&&region| has_deep_below(generation, region))
                        .map(|&region| {
                            let region_state = tree.node(region);
                            let field = region_state.children_field.as_ref().expect("region");
                            let inner =
                                retire_function(region_state.children_enum.as_ref().expect("enum"));
                            quote! { #inner(#payload.#field, cx); }
                        });
                    quote! { #pattern => { #(#retire)* } }
                }
                _ => {
                    let nothing = variant_pattern(generation, child, Binding::Nothing);
                    quote! { #nothing => {} }
                }
            }
        });
        let documentation = format!(
            "Consumes the value of `{enum_name}` that left and records its deep histories."
        );
        functions.extend(quote! {
            #[doc = #documentation]
            pub fn #name<#sink_parameter>(old: #enum_name, cx: &mut #cx_type) {
                match old { #(#arms)* }
            }
        });
    }
    functions
}
