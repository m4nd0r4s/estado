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

/// `complete`: finds the compound whose active substate is a final one, or
/// the parallel state whose regions are all final, and runs its `#[on_done]`
/// (spec 5.10, ADR 0005).
pub fn completion_function(generation: &Generation<'_>) -> TokenStream {
    let root = generation.root;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let arms = completion_arms(generation, 0, Mode::Direct);
    quote! {
        /// The synthetic completion microstep of spec 5.10: runs `#[on_done]`
        /// of the compound whose active substate is final, or of the parallel
        /// state whose regions are all final.
        pub fn complete<#sink_parameter>(
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

/// Where the arms are used: at the top (`Outcome`, `Stayed` when nothing is
/// complete) or inside a region (`Option<Outcome>`, so the parallel state
/// can tell that a completion inside a region ran).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Direct,
    InRegion,
}

impl Mode {
    fn found(self, outcome: TokenStream) -> TokenStream {
        match self {
            Self::Direct => outcome,
            Self::InRegion => quote! { ::core::option::Option::Some(#outcome) },
        }
    }

    fn nothing(self) -> TokenStream {
        match self {
            Self::Direct => quote! { ::estado_core::Outcome::Stayed },
            Self::InRegion => quote! { ::core::option::Option::None },
        }
    }
}

/// Whether a compound state below `owner` has an `#[on_done]` handler.
fn has_completion_below(generation: &Generation<'_>, owner: StateId) -> bool {
    let tree = generation.tree;
    tree.node(owner).children.iter().any(|&child| {
        (generation.handler(HandlerRole::OnDone, child).is_some()
            && matches!(
                tree.node(child).kind,
                StateKind::Compound | StateKind::Parallel
            ))
            || has_completion_below(generation, child)
    })
}

fn completion_arms(generation: &Generation<'_>, owner: StateId, mode: Mode) -> TokenStream {
    let tree = generation.tree;
    let bubble_pattern = generation.bubble_pattern();
    let arms = tree.node(owner).children.iter().filter_map(|&child| {
        let state = tree.node(child);
        match state.kind {
            StateKind::Compound => {
                if !has_completion_below(generation, child)
                    && generation.handler(HandlerRole::OnDone, child).is_none()
                {
                    return None;
                }
                let pattern = variant_pattern(generation, child, Binding::Everything);
                let payload = payload_binding(tree, child);
                let fields = fields_binding(tree, child);
                let substates = children_binding(tree, child);
                let inner = completion_arms(generation, child, mode);
                let nothing = mode.nothing();
                Some(quote! {
                    #pattern => {
                        let (#fields, #substates) = #payload.split();
                        match #substates {
                            #inner
                            _ => #nothing,
                        }
                    }
                })
            }
            StateKind::Parallel => Some(parallel_arm(generation, child)),
            _ => {
                let handler = generation.handler(HandlerRole::OnDone, owner)?;
                if !state.is_final {
                    return None;
                }
                let pattern = variant_pattern(generation, child, Binding::Everything);
                let payload = payload_binding(tree, child);
                let output =
                    matches!(state.payload, Payload::Newtype(_)).then(|| quote! { #payload });
                let fields = |ancestor: StateId| {
                    let binding = fields_binding(tree, ancestor);
                    quote! { #binding }
                };
                let call = call_handler(generation, handler, &fields, output.as_ref());
                let found = mode.found(quote! { outcome });
                let found_stayed = mode.found(quote! { ::estado_core::Outcome::Stayed });
                Some(quote! {
                    #pattern => {
                        let next = #call;
                        match resolve(next, slot, cx) {
                            Resolution::Done(outcome) => #found,
                            #bubble_pattern => {
                                #found_stayed
                            }
                        }
                    }
                })
            }
        }
    });
    quote! { #(#arms)* }
}

/// A parallel state: completions inside its regions first (one per
/// microstep), then its own `#[on_done]` when every region is final.
fn parallel_arm(generation: &Generation<'_>, parallel: StateId) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(parallel);
    let pattern = variant_pattern(generation, parallel, Binding::Everything);
    let payload = payload_binding(tree, parallel);
    let fields = fields_binding(tree, parallel);
    let bindings: Vec<_> = state
        .children
        .iter()
        .map(|&region| children_binding(tree, region))
        .collect();
    let own_handler = generation.handler(HandlerRole::OnDone, parallel);
    let bubble_pattern = generation.bubble_pattern();

    let nested = state.children.iter().filter_map(|&region| {
        if !has_completion_below(generation, region) {
            return None;
        }
        let arms = completion_arms(generation, region, Mode::InRegion);
        let binding = children_binding(tree, region);
        // The parallel state may complete right after: look again next time.
        let again = own_handler
            .is_some()
            .then(|| quote! { cx.runtime_mut().completion_pending = true; });
        Some(quote! {
            if let ::core::option::Option::Some(outcome) = match &mut *#binding {
                #arms
                _ => ::core::option::Option::None,
            } {
                #again
                return outcome;
            }
        })
    });

    let own = own_handler.and_then(|handler| {
        let conditions: Option<Vec<TokenStream>> = state
            .children
            .iter()
            .map(|&region| {
                let finals: Vec<TokenStream> = tree
                    .node(region)
                    .children
                    .iter()
                    .filter(|&&child| tree.node(child).is_final)
                    .map(|&child| variant_pattern(generation, child, Binding::Nothing))
                    .collect();
                let binding = children_binding(tree, region);
                (!finals.is_empty()).then(|| quote! { ::core::matches!(&*#binding, #(#finals)|*) })
            })
            .collect();
        let conditions = conditions?;
        let fields_argument = |ancestor: StateId| {
            let binding = fields_binding(tree, ancestor);
            quote! { #binding }
        };
        let call = call_handler(generation, handler, &fields_argument, None);
        Some(quote! {
            if #(#conditions)&&* {
                let next = #call;
                return match resolve(next, slot, cx) {
                    Resolution::Done(outcome) => outcome,
                    #bubble_pattern => ::estado_core::Outcome::Stayed,
                };
            }
        })
    });
    quote! {
        #pattern => {
            let (mut #fields, #(#bindings),*) = #payload.split();
            #(#nested)*
            #own
            ::estado_core::Outcome::Stayed
        }
    }
}
