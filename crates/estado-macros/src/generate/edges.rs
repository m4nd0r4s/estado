//! Edge enums and their `Edge::apply` (spec 5.3, ADR 0001, ADR 0011): one
//! enum per handler, one variant per `goto!`, linear transition code per
//! variant. The in-place rule (BM-01, ADR 0009): the simple form writes the
//! value of the LCA child in place; only the constructor form takes the slot.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use super::context::Generation;
use super::naming::{enter_function, exit_function, exiting_struct, payload_binding};
use super::tasks::{cancel_call, start_call};
use super::walk::{
    Binding, borrowed_walk, fields_references, owned_pattern, owned_rebuild, variant_pattern,
};
use crate::construct::exiting_type;
use crate::model::{
    GotoForm, GotoOccurrence, HandlerModel, HistoryKind, Payload, StateId, TransitionKind,
};
use crate::naming::state_constant_name;

/// The edge enums of every handler that has a `goto!`.
pub fn edge_items(generation: &Generation<'_>) -> TokenStream {
    generation
        .model
        .analysed_handlers
        .iter()
        .filter(|handler| !handler.gotos.is_empty())
        .map(|handler| edge_enum(generation, handler))
        .collect()
}

/// Name of the generic parameter holding a constructor closure:
/// `ToFailed` -> `FailedConstructor`.
fn constructor_parameter(goto: &GotoOccurrence) -> Ident {
    let variant = goto.variant.to_string();
    format_ident!("{}Constructor", variant.trim_start_matches("To"))
}

/// `Root::PLAYING`.
fn state_constant(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let root = generation.root;
    let constant = format_ident!("{}", state_constant_name(&generation.tree.node(node).name));
    quote! { #root::#constant }
}

/// The type of the value of the child of `lca`: its children enum.
fn children_type<'model>(generation: &Generation<'model>, lca: StateId) -> &'model Ident {
    generation
        .tree
        .node(lca)
        .children_enum
        .as_ref()
        .expect("the LCA owns a children enum")
}

fn edge_enum(generation: &Generation<'_>, handler: &HandlerModel) -> TokenStream {
    let tree = generation.tree;
    let root = generation.root;
    let name = &handler.edges_enum;
    let handler_state = &tree.node(handler.state).name;

    let constructors: Vec<&GotoOccurrence> = handler
        .gotos
        .iter()
        .filter(|goto| matches!(goto.form, GotoForm::Constructor { .. }))
        .collect();
    let parameters: Vec<Ident> = constructors
        .iter()
        .map(|goto| constructor_parameter(goto))
        .collect();
    let bounds_list: Vec<TokenStream> = constructors
        .iter()
        .map(|goto| {
            let GotoForm::Constructor { exiting } = goto.form else {
                unreachable!("filtered")
            };
            let parameter = constructor_parameter(goto);
            let exiting_ty = exiting_type(tree, exiting);
            let returned = children_type(generation, goto.lca);
            quote! { #parameter: ::core::ops::FnOnce(#exiting_ty) -> #returned }
        })
        .collect();
    let bounds = bounds_list.iter();

    let variants = handler.gotos.iter().map(|goto| {
        let variant = &goto.variant;
        let target = &tree.node(goto.target).name;
        match goto.form {
            GotoForm::Value => {
                let value = children_type(generation, goto.lca);
                let documentation =
                    format!("`goto!({target})`: carries the new value of the child of the LCA.");
                quote! { #[doc = #documentation] #variant(#value) }
            }
            GotoForm::Constructor { .. } => {
                let parameter = constructor_parameter(goto);
                let documentation =
                    format!("`goto!({target}, |old| ..)`: carries the constructor.");
                quote! { #[doc = #documentation] #variant(#parameter) }
            }
        }
    });
    let targets = handler
        .gotos
        .iter()
        .map(|goto| state_constant(generation, goto.target));
    let arms = handler
        .gotos
        .iter()
        .map(|goto| apply_arm(generation, goto, handler.state));
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let generics = if parameters.is_empty() {
        TokenStream::new()
    } else {
        quote! { <#(#parameters),*> }
    };
    let documentation =
        format!("The edges of the handler of `{handler_state}`, one variant per `goto!`.");
    let region_edge = region_edge_impl(generation, handler, &generics, &bounds_list);
    quote! {
        #[doc = #documentation]
        #[allow(clippy::enum_variant_names)]
        pub enum #name #generics {
            #(#variants,)*
        }

        impl<#(#bounds),*> ::estado_core::Edge<#root> for #name #generics {
            const TARGETS: &'static [::estado_core::StateId] = &[#(#targets),*];

            fn apply<#sink_parameter>(
                self,
                slot: &mut ::estado_core::Slot<#root>,
                cx: &mut #cx_type,
            ) -> bool {
                match self {
                    #(#arms)*
                }
            }
        }

        #region_edge
    }
}

/// `impl RegionEdge`: for a handler inside a region, which of its edges
/// leave the parallel state (spec 5.6 item 3).
fn region_edge_impl(
    generation: &Generation<'_>,
    handler: &HandlerModel,
    generics: &TokenStream,
    bounds: &[TokenStream],
) -> TokenStream {
    let tree = generation.tree;
    let Some(region) = tree.region_of(handler.state) else {
        return TokenStream::new();
    };
    let name = &handler.edges_enum;
    let arms = handler.gotos.iter().map(|goto| {
        let variant = &goto.variant;
        let leaves = !tree.is_ancestor_or_self(region, goto.lca);
        quote! { Self::#variant(..) => #leaves, }
    });
    quote! {
        impl<#(#bounds),*> RegionEdge for #name #generics {
            fn leaves_parallel(&self) -> bool {
                match self {
                    #(#arms)*
                }
            }
        }
    }
}

/// The code of one variant: exits, take or write, constructor, entries.
fn apply_arm(
    generation: &Generation<'_>,
    goto: &GotoOccurrence,
    handler_state: StateId,
) -> TokenStream {
    let tree = generation.tree;
    let root = generation.root;
    let variant = &goto.variant;
    // Inside a region the active leaf is the region's, not the machine's.
    let leaf = match tree
        .region_of(handler_state)
        .filter(|&region| tree.is_ancestor_or_self(region, goto.lca))
    {
        Some(region) => {
            let function = super::regions::region_leaf_function(generation, region);
            quote! { #function(slot) }
        }
        None => quote! { <#root as ::estado_core::Spec>::state_id(slot.live_mut()) },
    };
    let before = goto
        .compares_leaf
        .then(|| quote! { let leaf_before = #leaf; });
    let changed = if goto.compares_leaf {
        quote! { #leaf != leaf_before }
    } else {
        quote! { true }
    };
    let exit = exit_function(tree, goto.lca);
    let enter = enter_function(tree, goto.lca);
    let walk = borrowed_walk(generation, goto.lca);
    let prelude = &walk.prelude;
    let children = &walk.children;
    let ancestors = fields_references(generation, &walk.chain);
    let cancel = cancel_call(generation, goto.lca, children);
    let start = start_call(generation, goto.lca, children);

    match goto.form {
        GotoForm::Value => {
            let write = if super::history::has_deep_below(generation, goto.lca) {
                let retire = super::history::retire_function(children_type(generation, goto.lca));
                quote! { #retire(::core::mem::replace(#children, value), cx); }
            } else {
                quote! { *#children = value; }
            };
            let restore = (goto.kind == TransitionKind::History)
                .then(|| history_override(generation, goto.target));
            let (leave, arrive) = if restore.is_some() {
                (
                    quote! { { #prelude #exit(#children, #(#ancestors,)* cx); #cancel #write } },
                    quote! { { #prelude #enter(#children, #(#ancestors,)* cx); #start } },
                )
            } else {
                (
                    quote! { #prelude #exit(#children, #(#ancestors,)* cx); #cancel #write },
                    quote! { #enter(#children, #(#ancestors,)* cx); #start },
                )
            };
            quote! {
                Self::#variant(value) => {
                    #before
                    #leave
                    #restore
                    #arrive
                    #changed
                }
            }
        }
        GotoForm::Constructor { exiting } => {
            let old_children = format_ident!("old_children");
            let pattern = owned_pattern(generation, goto.lca, &old_children);
            let take = if goto.lca == 0 {
                quote! { let #old_children = slot.take(); }
            } else {
                quote! { let #pattern = slot.take() else { ::core::unreachable!("the exiting path is active") }; }
            };
            let new_children = format_ident!("new_children");
            let rebuilt = owned_rebuild(generation, goto.lca, quote! { #new_children });
            let owner_pattern = variant_pattern(generation, exiting, Binding::Everything);
            let conversion = exiting_conversion(generation, exiting);
            quote! {
                Self::#variant(constructor) => {
                    #before
                    {
                        #prelude
                        #exit(#children, #(#ancestors,)* cx);
                        #cancel
                    }
                    #take
                    let exiting = match #old_children {
                        #owner_pattern => #conversion,
                        #[allow(unreachable_patterns)]
                        _ => ::core::unreachable!("the exiting state is active"),
                    };
                    let #new_children = constructor(exiting);
                    slot.put(#rebuilt);
                    {
                        #prelude
                        #enter(#children, #(#ancestors,)* cx);
                        #start
                    }
                    #changed
                }
            }
        }
    }
}

/// The owned `Exiting![X]` value built from the bindings of the pattern.
fn exiting_conversion(generation: &Generation<'_>, exiting: StateId) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(exiting);
    let structure = exiting_struct(tree, exiting);
    match &state.payload {
        Payload::Newtype(_) if state.history == HistoryKind::DeepKeep => {
            // The substate moves into the history; the constructor gets the rest.
            let payload = payload_binding(tree, exiting);
            let record = super::history::record_deep(generation, exiting, &quote! { #payload });
            let shared = tree.shared_fields(exiting);
            let pairs = shared
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .map(|name| {
                    let binding = super::naming::field_binding(tree, exiting, &name.to_string());
                    quote! { #name: #binding }
                });
            let struct_name = state.composite_struct.as_ref().expect("a compound");
            let remaining = format_ident!("{}Exiting", struct_name);
            quote! { { #record #remaining { #(#pairs),* } } }
        }
        Payload::Newtype(_) => {
            let payload = payload_binding(tree, exiting);
            quote! { #payload }
        }
        Payload::Struct(fields) => {
            let pairs = fields
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .map(|name| {
                    let binding = super::naming::field_binding(tree, exiting, &name.to_string());
                    quote! { #name: #binding }
                });
            quote! { #structure { #(#pairs),* } }
        }
        Payload::Tuple(fields) => {
            let names = (0..fields.len())
                .map(|index| super::naming::field_binding(tree, exiting, &index.to_string()));
            quote! { #structure(#(#names),*) }
        }
        Payload::Unit | Payload::None => quote! { () },
    }
}

/// After the new value is written, replaces the substates of the history
/// target with the recorded ones (`goto!(T, history)`): deep history moves
/// the recorded value in, shallow history rebuilds it from the discriminant.
/// Without a record the `#[initial]` the edge carries stays.
fn history_override(generation: &Generation<'_>, target: StateId) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(target);
    let walk = borrowed_walk(generation, target);
    let prelude = &walk.prelude;
    let children = &walk.children;
    let field = super::runtime::history_field(generation, target);
    let apply = match state.history {
        HistoryKind::DeepKeep => quote! {
            if let ::core::option::Option::Some(recorded) = cx.runtime_mut().#field.take() {
                *#children = recorded;
            }
        },
        _ => {
            let restore = super::history::from_discriminant_function(
                state.children_enum.as_ref().expect("a compound"),
            );
            quote! {
                if let ::core::option::Option::Some(discriminant) = cx.runtime_mut().#field {
                    *#children = #restore(discriminant);
                }
            }
        }
    };
    quote! { { #prelude #apply } }
}
