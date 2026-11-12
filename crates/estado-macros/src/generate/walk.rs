//! Navigating the nested enums: patterns for variants, arguments for user
//! functions, and the borrowed and owned walks from the root down to a
//! compound state.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::context::Generation;
use super::naming::{children_binding, field_binding, fields_binding, payload_binding, view_type};
use crate::model::{HandlerModel, HandlerParameter, Payload, StateId, StateKind};

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

/// A borrowed walk from the slot down to a state that owns children: the
/// root, a compound state, a parallel state or a region.
pub struct Walk {
    /// `let` statements binding the `*Fields` of every compound and parallel
    /// state on the way, the regions of the parallel states and the children
    /// enum of `at`.
    pub prelude: TokenStream,
    /// The states with a `*Fields` view on the way, top down (ending with
    /// `at` when it has one): the ancestors argument list of the entry and
    /// exit functions.
    pub chain: Vec<StateId>,
    /// The binding of the `&mut` children enum of `at` (a compound state,
    /// a region or the root).
    pub children: Ident,
}

/// The statement that destructures the variant of `node` and splits its
/// borrow: `(fields, substates)` of a compound, `(fields, region, ..)` of a
/// parallel state.
fn split_statement(
    generation: &Generation<'_>,
    node: StateId,
    scrutinee: &TokenStream,
) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(node);
    let pattern = variant_pattern(generation, node, Binding::Everything);
    let payload = payload_binding(tree, node);
    let fields = fields_binding(tree, node);
    let message = format!("`{}` is active", state.name);
    let parts: Vec<Ident> = match state.kind {
        StateKind::Parallel => state
            .children
            .iter()
            .map(|&region| children_binding(tree, region))
            .collect(),
        _ => vec![children_binding(tree, node)],
    };
    quote! {
        let #pattern = #scrutinee else { ::core::unreachable!(#message) };
        let (mut #fields, #(#parts),*) = #payload.split();
    }
}

/// Walks from `slot` to `at` (the root, a compound state, a parallel state
/// or a region), splitting the borrow at every level.
pub fn borrowed_walk(generation: &Generation<'_>, at: StateId) -> Walk {
    let tree = generation.tree;
    let chain = chain_to(generation, at);
    let children = children_binding(tree, at);
    if chain.is_empty() {
        let root = generation.root;
        return Walk {
            prelude: quote! { let #children: &mut #root = slot.live_mut(); },
            chain: Vec::new(),
            children,
        };
    }
    let mut statements = TokenStream::new();
    let mut scrutinee = quote! { slot.live_mut() };
    for &node in &chain {
        match tree.node(node).kind {
            // Bound by the `split` of the parallel state above it.
            StateKind::Region => {
                scrutinee = {
                    let region = children_binding(tree, node);
                    quote! { #region }
                }
            }
            _ => {
                statements.extend(split_statement(generation, node, &scrutinee));
                let substates = children_binding(tree, node);
                scrutinee = quote! { #substates };
            }
        }
    }
    Walk {
        prelude: statements,
        chain: tree.fields_chain(at),
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

/// Nests the variants of `chain` (top down) around `innermost`. The same
/// text is a pattern when destructuring and an expression when rebuilding;
/// the regions of a parallel state off the path are bound by name and moved
/// back.
fn owned_levels(
    generation: &Generation<'_>,
    chain: &[StateId],
    innermost: TokenStream,
) -> TokenStream {
    let Some((&node, rest)) = chain.split_first() else {
        return innermost;
    };
    let tree = generation.tree;
    let state = tree.node(node);
    let struct_name = state.composite_struct.as_ref().expect("a composite state");
    let shared = tree.shared_fields(node);
    let mut pairs: Vec<TokenStream> = shared
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .map(|name| {
            let binding = field_binding(tree, node, &name.to_string());
            quote! { #name: #binding }
        })
        .collect();
    if state.kind == StateKind::Parallel {
        let on_path = rest.first().copied();
        let below = owned_levels(generation, rest.get(1..).unwrap_or(&[]), innermost);
        for &region in &state.children {
            let field = tree.node(region).children_field.as_ref().expect("region");
            if Some(region) == on_path {
                pairs.push(quote! { #field: #below });
            } else {
                let binding = field_binding(tree, node, &field.to_string());
                pairs.push(quote! { #field: #binding });
            }
        }
    } else {
        let children_field = state.children_field.as_ref().expect("substates field");
        let below = owned_levels(generation, rest, innermost);
        pairs.push(quote! { #children_field: #below });
    }
    let path = variant_path(generation, node);
    quote! { #path(#struct_name { #(#pairs,)* }) }
}
