//! Navigating the nested enums: patterns for variants, arguments for user
//! functions, and the borrowed and owned walks from the root down to a
//! compound state.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::context::Generation;
use super::naming::{children_binding, field_binding, fields_binding, payload_binding, view_type};
use crate::model::{HandlerModel, HandlerParameter, Payload, StateId};

/// Which parts of a variant a pattern binds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// `E::V(..)`, `E::V { .. }`.
    Nothing,
    /// Every field, under the names of [`field_binding`] and
    /// [`payload_binding`].
    Everything,
}

fn variant_path(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let state = generation.tree.node(node);
    let owner = state.owner_enum.as_ref().expect("a variant has an owner");
    let variant = &state.name;
    quote! { #owner::#variant }
}

/// The pattern of a variant of its owner enum.
pub fn variant_pattern(
    generation: &Generation<'_>,
    node: StateId,
    binding: Binding,
) -> TokenStream {
    let tree = generation.tree;
    let path = variant_path(generation, node);
    match (&tree.node(node).payload, binding) {
        (Payload::Unit | Payload::None, _) => path,
        (Payload::Newtype(_), Binding::Nothing) | (Payload::Tuple(_), Binding::Nothing) => {
            quote! { #path(..) }
        }
        (Payload::Struct(_), Binding::Nothing) => quote! { #path { .. } },
        (Payload::Newtype(_), Binding::Everything) => {
            let payload = payload_binding(tree, node);
            quote! { #path(#payload) }
        }
        (Payload::Tuple(fields), Binding::Everything) => {
            let names =
                (0..fields.len()).map(|index| field_binding(tree, node, &index.to_string()));
            quote! { #path(#(#names),*) }
        }
        (Payload::Struct(fields), Binding::Everything) => {
            let pairs = fields
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .map(|name| {
                    let binding = field_binding(tree, node, &name.to_string());
                    quote! { #name: #binding }
                });
            quote! { #path { #(#pairs),* } }
        }
    }
}

/// `PlayingMut { pos: playing_pos }` from the bindings of
/// [`Binding::Everything`].
pub fn own_view(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    let view = view_type(tree, node);
    match &tree.node(node).payload {
        Payload::Struct(fields) => {
            let pairs = fields
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .map(|name| {
                    let binding = field_binding(tree, node, &name.to_string());
                    quote! { #name: #binding }
                });
            quote! { #view { #(#pairs),* } }
        }
        Payload::Tuple(fields) => {
            let names =
                (0..fields.len()).map(|index| field_binding(tree, node, &index.to_string()));
            quote! { #view(#(#names),*) }
        }
        _ => quote! { () },
    }
}

/// The arguments of a call to a user function, in the order of its
/// parameters. `fields` gives the expression for the `*Fields` view of a
/// compound state; the own data comes from the bindings of the variant.
pub fn handler_arguments(
    generation: &Generation<'_>,
    handler: &HandlerModel,
    fields: &dyn Fn(StateId) -> TokenStream,
    output: Option<&TokenStream>,
) -> Vec<TokenStream> {
    handler
        .parameters
        .iter()
        .map(|parameter| match parameter {
            HandlerParameter::OwnData { .. } => {
                let payload = payload_binding(generation.tree, handler.state);
                quote! { #payload }
            }
            HandlerParameter::OwnView => own_view(generation, handler.state),
            HandlerParameter::Fields { ancestor } => fields(*ancestor),
            HandlerParameter::Output => output
                .cloned()
                .unwrap_or_else(|| quote! { ::core::unreachable!("no output here") }),
            HandlerParameter::Context => quote! { cx },
            HandlerParameter::Event => quote! { event },
        })
        .collect()
}

/// `Root::on_playing::<EffectSink>(arguments)`.
pub fn call_handler(
    generation: &Generation<'_>,
    handler: &HandlerModel,
    fields: &dyn Fn(StateId) -> TokenStream,
    output: Option<&TokenStream>,
) -> TokenStream {
    let root = generation.root;
    let name = &handler.function.sig.ident;
    let sink = generation.sink();
    let arguments = handler_arguments(generation, handler, fields, output);
    quote! { #root::#name::<#sink>(#(#arguments),*) }
}

/// The compound states from the top down to `at` (inclusive); empty for the
/// root.
pub fn chain_to(generation: &Generation<'_>, at: StateId) -> Vec<StateId> {
    let mut chain = generation.tree.ancestors(at);
    chain.reverse();
    chain
}

/// A borrowed walk from the slot down to the children enum of `at`.
pub struct Walk {
    /// `let` statements binding the `*Fields` of every compound state on the
    /// way and the children enum of `at`.
    pub prelude: TokenStream,
    /// The compound states on the way, top down, ending with `at`.
    pub chain: Vec<StateId>,
    /// The binding of `&mut` children enum of `at`.
    pub children: Ident,
}

/// Walks from `slot` to the children enum of `at` (the root or a compound
/// state), splitting the borrow at every level.
pub fn borrowed_walk(generation: &Generation<'_>, at: StateId) -> Walk {
    let tree = generation.tree;
    let chain = chain_to(generation, at);
    let children = children_binding(tree, at);
    if chain.is_empty() {
        let root = generation.root;
        return Walk {
            prelude: quote! { let #children: &mut #root = slot.live_mut(); },
            chain,
            children,
        };
    }
    let mut statements = TokenStream::new();
    let mut scrutinee = quote! { slot.live_mut() };
    for &node in &chain {
        let pattern = variant_pattern(generation, node, Binding::Everything);
        let payload = payload_binding(tree, node);
        let fields = fields_binding(tree, node);
        let substates = children_binding(tree, node);
        let message = format!("`{}` is active", tree.node(node).name);
        statements.extend(quote! {
            let #pattern = #scrutinee else { ::core::unreachable!(#message) };
            let (mut #fields, #substates) = #payload.split();
        });
        scrutinee = quote! { #substates };
    }
    Walk {
        prelude: statements,
        chain,
        children,
    }
}

/// `&mut active_fields, &mut playing_fields`: the ancestors argument list of
/// the entry and exit functions.
pub fn fields_references(generation: &Generation<'_>, chain: &[StateId]) -> Vec<TokenStream> {
    chain
        .iter()
        .map(|&node| {
            let fields = fields_binding(generation.tree, node);
            quote! { &mut #fields }
        })
        .collect()
}

/// The pattern that destructures the root by value down to the children
/// enum of `at`, binding it as `old_children`.
pub fn owned_pattern(
    generation: &Generation<'_>,
    at: StateId,
    old_children: &Ident,
) -> TokenStream {
    let chain = chain_to(generation, at);
    owned_levels(generation, &chain, quote! { #old_children })
}

/// The expression that rebuilds the root around `new_children`.
pub fn owned_rebuild(
    generation: &Generation<'_>,
    at: StateId,
    new_children: TokenStream,
) -> TokenStream {
    let chain = chain_to(generation, at);
    owned_levels(generation, &chain, new_children)
}

fn owned_levels(
    generation: &Generation<'_>,
    chain: &[StateId],
    innermost: TokenStream,
) -> TokenStream {
    let tree = generation.tree;
    let mut inner = innermost;
    for &node in chain.iter().rev() {
        let state = tree.node(node);
        let struct_name = state.composite_struct.as_ref().expect("a compound state");
        let children_field = state.children_field.as_ref().expect("substates field");
        let shared = tree.shared_fields(node);
        let pairs = shared
            .iter()
            .filter_map(|field| field.ident.as_ref())
            .map(|name| {
                let binding = field_binding(tree, node, &name.to_string());
                quote! { #name: #binding }
            });
        let path = variant_path(generation, node);
        inner = quote! { #path(#struct_name { #(#pairs,)* #children_field: #inner }) };
    }
    inner
}
