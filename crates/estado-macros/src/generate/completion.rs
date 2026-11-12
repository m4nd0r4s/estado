//! Final states (spec 5.10, ADR 0005, ADR 0013 item 4): the completion
//! microstep that runs `#[on_done]`, and `is_done` / `into_output` for a
//! `#[terminal]` root state.
//!
//! Every state that can complete has its own pending flag in the runtime:
//! set by the entry of a `#[terminal]` that completes it, cleared when
//! `run_always` serves it or when the state exits. A completion therefore
//! fires exactly once per entry into a final configuration, and a sibling
//! region or a nested compound completing does not run it again.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

use super::context::Generation;
use super::naming::{children_binding, payload_binding};
use super::runtime::done_field;
use super::walk::{Binding, FieldsAccess, call_handler, state_function, variant_pattern};
use crate::model::{HandlerRole, Payload, StateId, StateKind, StateTree};

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

/// Whether the state can complete: a compound state with a `#[terminal]`
/// substate, or a parallel state with one in some region.
fn can_complete(tree: &StateTree, node: StateId) -> bool {
    let has_terminal = |owner: StateId| {
        tree.node(owner)
            .children
            .iter()
            .any(|&child| tree.node(child).is_final)
    };
    match tree.node(node).kind {
        StateKind::Compound => has_terminal(node),
        StateKind::Parallel => tree
            .node(node)
            .children
            .iter()
            .any(|&region| has_terminal(region)),
        _ => false,
    }
}

/// The states with a pending flag, deepest first: an inner completion runs
/// before the one of the state around it.
pub fn completing_states(generation: &Generation<'_>) -> Vec<StateId> {
    let tree = generation.tree;
    let mut states: Vec<StateId> = tree
        .nodes
        .iter()
        .skip(1)
        .map(|node| node.id)
        .filter(|&node| can_complete(tree, node))
        .collect();
    states.sort_by_key(|&node| std::cmp::Reverse((tree.node(node).depth, node)));
    states
}

/// The state whose pending flag entering `terminal` sets: its parent
/// compound, or the parallel state of its region. `None` for a root
/// `#[terminal]`, which only makes `is_done()` true.
pub fn completed_by(tree: &StateTree, terminal: StateId) -> Option<StateId> {
    let parent = tree.node(terminal).parent.filter(|&parent| parent != 0)?;
    match tree.node(parent).kind {
        StateKind::Region => tree.node(parent).parent,
        _ => Some(parent),
    }
}

/// `complete_<state>` for every completing state that has an `#[on_done]`.
pub fn completion_functions(generation: &Generation<'_>) -> TokenStream {
    completing_states(generation)
        .into_iter()
        .filter_map(|node| {
            let handler = generation.handler(HandlerRole::OnDone, node)?;
            let body = if generation.tree.node(node).kind == StateKind::Parallel {
                parallel_body(generation, node, handler)
            } else {
                compound_body(generation, node, handler)
            };
            let name = complete_function(generation, node);
            let documentation = format!(
                "The completion microstep of `{}`.",
                generation.tree.node(node).name
            );
            Some(state_function(
                generation,
                node,
                &name,
                &documentation,
                body,
            ))
        })
        .collect()
}

/// `complete_upload`.
pub fn complete_function(generation: &Generation<'_>, node: StateId) -> proc_macro2::Ident {
    quote::format_ident!(
        "complete_{}",
        crate::naming::snake_case_name(&generation.tree.node(node).name)
    )
}

/// The statements of `run_always` that serve the pending completions.
pub fn serve_completions(generation: &Generation<'_>) -> TokenStream {
    completing_states(generation)
        .into_iter()
        .map(|node| {
            let flag = done_field(generation, node);
            let served = if generation.handler(HandlerRole::OnDone, node).is_some() {
                let function = complete_function(generation, node);
                quote! { return #function(slot, cx); }
            } else {
                // No `#[on_done]`: the check runs as a microstep and finds
                // nothing to do (ADR 0005).
                quote! { return __estado_core::Outcome::Stayed; }
            };
            quote! {
                if ::core::mem::take(&mut cx.runtime_mut().#flag) {
                    #served
                }
            }
        })
        .collect()
}

/// What a completion does with the `Next` of its handler. Only `stay!()` and
/// `goto!` are allowed there (E0045), so nothing else bubbles.
fn run_handler(generation: &Generation<'_>, call: &TokenStream) -> TokenStream {
    let bubble_pattern = generation.bubble_pattern();
    quote! {
        let next = #call;
        match resolve(next, slot, cx) {
            Resolution::Done(outcome) => outcome,
            #bubble_pattern => __estado_core::Outcome::Stayed,
        }
    }
}

/// A compound state: it completed when its active substate is a `#[terminal]`.
fn compound_body(
    generation: &Generation<'_>,
    node: StateId,
    handler: &crate::model::HandlerModel,
) -> TokenStream {
    let tree = generation.tree;
    let substates = children_binding(tree, node);
    let arms = tree
        .node(node)
        .children
        .iter()
        .filter(|&&child| tree.node(child).is_final)
        .map(|&child| {
            let state = tree.node(child);
            let pattern = variant_pattern(generation, child, Binding::Everything);
            let payload = payload_binding(tree, child);
            let output = matches!(state.payload, Payload::Newtype(_)).then(|| quote! { #payload });
            let call = call_handler(generation, handler, FieldsAccess::Binding, output.as_ref());
            let run = run_handler(generation, &call);
            quote! { #pattern => { #run } }
        });
    quote! {
        match #substates {
            #(#arms)*
            #[allow(unreachable_patterns)]
            _ => __estado_core::Outcome::Stayed,
        }
    }
}

/// A parallel state: it completed when every region is in a `#[terminal]`;
/// a region without one never completes.
fn parallel_body(
    generation: &Generation<'_>,
    node: StateId,
    handler: &crate::model::HandlerModel,
) -> TokenStream {
    let tree = generation.tree;
    let conditions: Option<Vec<TokenStream>> = tree
        .node(node)
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
    let Some(conditions) = conditions else {
        return quote! { __estado_core::Outcome::Stayed };
    };
    let call = call_handler(generation, handler, FieldsAccess::Binding, None);
    let run = run_handler(generation, &call);
    quote! {
        if #(#conditions)&&* {
            #run
        } else {
            __estado_core::Outcome::Stayed
        }
    }
}
