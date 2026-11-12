//! Final states (spec 5.10, ADR 0005): the completion microstep that runs
//! `#[on_done]`, and `is_done` / `into_output` for a `#[terminal]` root state.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

use super::context::Generation;
use super::naming::{children_binding, fields_binding, payload_binding};
use super::walk::{Binding, call_handler, variant_pattern};
use crate::model::{HandlerRole, Payload, StateId, StateKind};

/// The `Output` of the machine: the payload type shared by the root
/// `#[terminal]` variants, or `()`.
pub fn output_type(generation: &Generation<'_>) -> Type {
    let tree = generation.tree;
    tree.root()
        .children
        .iter()
        .map(|&child| tree.node(child))
        .filter(|node| node.is_final)
        .find_map(|node| match &node.payload {
            Payload::Newtype(payload) => Some(payload.clone()),
            _ => None,
        })
        .unwrap_or_else(|| syn::parse_quote!(()))
}

/// `fn is_done` and `fn into_output` of the `Spec` impl, when the root has
/// `#[terminal]` variants.
pub fn done_methods(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let root = generation.root;
    let finals: Vec<StateId> = tree
        .root()
        .children
        .iter()
        .copied()
        .filter(|&child| tree.node(child).is_final)
        .collect();
    if finals.is_empty() {
        return TokenStream::new();
    }
    let patterns: Vec<TokenStream> = finals
        .iter()
        .map(|&node| variant_pattern(generation, node, Binding::Nothing))
        .collect();
    let output = output_type(generation);
    let unit_output = matches!(&output, Type::Tuple(tuple) if tuple.elems.is_empty());
    let arms: Vec<TokenStream> = finals
        .iter()
        .filter_map(|&node| {
            if unit_output {
                let pattern = variant_pattern(generation, node, Binding::Nothing);
                return Some(quote! { #pattern => ::core::result::Result::Ok(()), });
            }
            let Payload::Newtype(_) = tree.node(node).payload else {
                return None;
            };
            let pattern = variant_pattern(generation, node, Binding::Everything);
            let payload = payload_binding(tree, node);
            Some(quote! { #pattern => ::core::result::Result::Ok(#payload), })
        })
        .collect();
    quote! {
        fn is_done(&self) -> bool {
            ::core::matches!(self, #(#patterns)|*)
        }

        fn into_output(self) -> ::core::result::Result<#output, #root> {
            match self {
                #(#arms)*
                other => ::core::result::Result::Err(other),
            }
        }
    }
}

/// Whether the machine has a completion microstep at all: some final state
/// below the root, or an `#[on_done]` handler.
pub fn has_completion(generation: &Generation<'_>) -> bool {
    generation
        .tree
        .nodes
        .iter()
        .skip(1)
        .any(|node| node.is_final && node.parent.unwrap_or(0) != 0)
        || generation
            .model
            .analysed_handlers
            .iter()
            .any(|handler| handler.role == HandlerRole::OnDone)
}

/// `run_always`: finds the compound whose active substate is a final one and
/// runs its `#[on_done]`.
pub fn completion_function(generation: &Generation<'_>) -> TokenStream {
    let root = generation.root;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let arms = completion_arms(generation, 0);
    quote! {
        /// The synthetic completion microstep of spec 5.10: runs `#[on_done]`
        /// of the compound whose active substate is final.
        pub fn run_always<#sink_parameter>(
            slot: &mut ::estado_core::Slot<#root>,
            cx: &mut #cx_type,
        ) -> ::estado_core::Outcome {
            match slot.live_mut() {
                #arms
                _ => ::estado_core::Outcome::Stayed,
            }
        }
    }
}

fn completion_arms(generation: &Generation<'_>, owner: StateId) -> TokenStream {
    let tree = generation.tree;
    let arms = tree.node(owner).children.iter().filter_map(|&child| {
        let state = tree.node(child);
        if state.kind == StateKind::Compound {
            let pattern = variant_pattern(generation, child, Binding::Everything);
            let payload = payload_binding(tree, child);
            let fields = fields_binding(tree, child);
            let substates = children_binding(tree, child);
            let inner = completion_arms(generation, child);
            return Some(quote! {
                #pattern => {
                    let (#fields, #substates) = #payload.split();
                    match #substates {
                        #inner
                        _ => ::estado_core::Outcome::Stayed,
                    }
                }
            });
        }
        let handler = generation.handler(HandlerRole::OnDone, owner)?;
        if !state.is_final {
            return None;
        }
        let pattern = variant_pattern(generation, child, Binding::Everything);
        let payload = payload_binding(tree, child);
        let output = matches!(state.payload, Payload::Newtype(_)).then(|| quote! { #payload });
        let fields = |ancestor: StateId| {
            let binding = fields_binding(tree, ancestor);
            quote! { #binding }
        };
        let call = call_handler(generation, handler, &fields, output.as_ref());
        Some(quote! {
            #pattern => {
                let next = #call;
                match resolve(next, slot, cx) {
                    Resolution::Done(outcome) => outcome,
                    Resolution::Bubble | Resolution::Reenter => ::estado_core::Outcome::Stayed,
                }
            }
        })
    });
    quote! { #(#arms)* }
}
