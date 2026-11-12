//! Bubbling (spec 5.1): what happens after a handler returned, the functions
//! that run the handler of an ancestor, and `reenter!()` (ADR 0006).

use proc_macro2::TokenStream;
use quote::quote;

use super::context::Generation;
use super::naming::{
    bubble_function, enter_function, exit_function, fields_binding, reenter_function,
};
use super::walk::{borrowed_walk, call_handler, fields_references};
use crate::construct::initial_value;
use crate::model::{HandlerRole, StateId, StateKind, UnhandledPolicy};

/// `::estado_core::UnhandledPolicy::Error` and its siblings.
fn policy_expression(policy: UnhandledPolicy) -> TokenStream {
    match policy {
        UnhandledPolicy::Ignore => quote! { ::estado_core::UnhandledPolicy::Ignore },
        UnhandledPolicy::Error => quote! { ::estado_core::UnhandledPolicy::Error },
        UnhandledPolicy::Panic => quote! { ::estado_core::UnhandledPolicy::Panic },
    }
}

/// `Outcome::Unhandled(policy)` with the machine default or an override.
pub fn unhandled_outcome(
    generation: &Generation<'_>,
    policy: Option<UnhandledPolicy>,
) -> TokenStream {
    let root = generation.root;
    match policy {
        None => {
            quote! { ::estado_core::Outcome::Unhandled(<#root as ::estado_core::Spec>::UNHANDLED) }
        }
        Some(policy) => {
            let policy = policy_expression(policy);
            quote! { ::estado_core::Outcome::Unhandled(#policy) }
        }
    }
}

/// The event left `node` unhandled: apply its policy override, or go on to
/// the nearest ancestor that has a handler (an `Outcome` expression).
pub fn bubble_out_of(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    let in_region = tree.region_of(node).is_some();
    if let Some(policy) = tree.node(node).unhandled {
        return if in_region {
            let policy = policy_expression(policy);
            quote! { tally.stop(#policy) }
        } else {
            unhandled_outcome(generation, Some(policy))
        };
    }
    let parent = tree.node(node).parent.unwrap_or(0);
    if parent == 0 || tree.node(parent).kind == StateKind::Region {
        // At the top of a region the event goes back to the parallel state,
        // which asks the other regions (spec 5.6).
        return unhandled_outcome(generation, None);
    }
    if generation.handler(HandlerRole::On, parent).is_some()
        && tree.node(parent).kind != StateKind::Parallel
    {
        let function = bubble_function(tree, parent);
        return if in_region {
            quote! { #function(slot, cx, event, tally) }
        } else {
            quote! { #function(slot, cx, event) }
        };
    }
    bubble_out_of(generation, parent)
}

/// Calls `call` (the handler of `node`), resolves its `Next` and applies the
/// bubbling rules. An expression of type `Outcome`. Inside a region the
/// `tally` of the parallel state is in scope (spec 5.6).
pub fn handle_at(generation: &Generation<'_>, node: StateId, call: TokenStream) -> TokenStream {
    let tree = generation.tree;
    let in_region = tree.region_of(node).is_some();
    let override_policy = tree.node(node).unhandled;
    let handler = generation
        .handler(HandlerRole::On, node)
        .expect("handle_at is called for a state with a handler");
    let mapped = override_policy.map(|policy| {
        let outcome = if in_region {
            let policy = policy_expression(policy);
            quote! { tally.stop(#policy) }
        } else {
            unhandled_outcome(generation, Some(policy))
        };
        quote! { Resolution::Done(::estado_core::Outcome::Unhandled(_)) => #outcome, }
    });
    let bubble = bubble_out_of(generation, node);
    let reenter = if !generation.uses_reenter() {
        TokenStream::new()
    } else if handler.uses.reenter {
        let function = reenter_function(tree, node);
        quote! { Resolution::Reenter => #function(slot, cx), }
    } else {
        quote! { Resolution::Reenter => ::core::unreachable!(), }
    };
    let resolve = if in_region {
        quote! { resolve_region(next, slot, cx, tally) }
    } else {
        quote! { resolve(next, slot, cx) }
    };
    quote! {
        {
            let next = #call;
            match #resolve {
                #mapped
                Resolution::Done(outcome) => outcome,
                Resolution::Bubble => #bubble,
                #reenter
            }
        }
    }
}

/// `bubble_to_<state>` for every compound state with a handler: re-derives
/// the borrows from the slot and runs the handler.
pub fn bubble_functions(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let event = &generation.model.options.event;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let root = generation.root;
    let mut functions = TokenStream::new();
    for node in tree.nodes.iter().skip(1) {
        if node.kind != StateKind::Compound {
            continue;
        }
        let Some(handler) = generation.handler(HandlerRole::On, node.id) else {
            continue;
        };
        let name = bubble_function(tree, node.id);
        // A handler that takes no data of any state needs no borrows.
        let needs_data = handler.parameters.iter().any(|parameter| {
            matches!(
                parameter,
                crate::model::HandlerParameter::OwnData { .. }
                    | crate::model::HandlerParameter::OwnView
                    | crate::model::HandlerParameter::Fields { .. }
            )
        });
        let walk = borrowed_walk(generation, node.id);
        let prelude = needs_data.then_some(&walk.prelude);
        let fields = |ancestor: StateId| {
            let binding = fields_binding(tree, ancestor);
            quote! { #binding }
        };
        let call = call_handler(generation, handler, &fields, None);
        let body = handle_at(generation, node.id, call);
        let documentation = format!(
            "Runs the handler of `{}` after its substates bubbled.",
            node.name
        );
        let tally = tree
            .region_of(node.id)
            .is_some()
            .then(|| quote! { , tally: &mut RegionTally });
        functions.extend(quote! {
            #[doc = #documentation]
            #[inline(always)]
            pub fn #name<#sink_parameter>(
                slot: &mut ::estado_core::Slot<#root>,
                cx: &mut #cx_type,
                event: &#event
                #tally
            ) -> ::estado_core::Outcome {
                #prelude
                #body
            }
        });
    }
    functions
}

/// `reenter_<state>` for every state whose handler uses `reenter!()`.
pub fn reenter_functions(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let root = generation.root;
    let mut functions = TokenStream::new();
    for handler in &generation.model.analysed_handlers {
        if handler.role != HandlerRole::On || !handler.uses.reenter {
            continue;
        }
        let node = handler.state;
        let parent = tree.node(node).parent.unwrap_or(0);
        let name = reenter_function(tree, node);
        let exit = exit_function(tree, parent);
        let enter = enter_function(tree, parent);
        let walk = borrowed_walk(generation, parent);
        let prelude = &walk.prelude;
        let children = &walk.children;
        let ancestors = fields_references(generation, &walk.chain);
        let reset = reset_statements(generation, node, children);
        let cancel = super::tasks::cancel_call(generation, parent, children);
        let start = super::tasks::start_call(generation, parent, children);
        let documentation = format!(
            "`reenter!()` of `{}`: exits and enters it, its data kept (ADR 0006).",
            tree.node(node).name
        );
        functions.extend(quote! {
            #[doc = #documentation]
            pub fn #name<#sink_parameter>(
                slot: &mut ::estado_core::Slot<#root>,
                cx: &mut #cx_type,
            ) -> ::estado_core::Outcome {
                let leaf_before = <#root as ::estado_core::Spec>::state_id(slot.live_mut());
                {
                    #prelude
                    #exit(#children, #(#ancestors,)* cx);
                    #cancel
                    #reset
                    #enter(#children, #(#ancestors,)* cx);
                    #start
                }
                ::estado_core::Outcome::Transitioned {
                    changed: <#root as ::estado_core::Spec>::state_id(slot.live_mut()) != leaf_before,
                }
            }
        });
    }
    functions
}

/// What `reenter!()` does to the substates of a compound state or to the
/// regions of a parallel one: they restart from their `#[initial]`, the
/// values that leave are consumed (deep histories record them).
fn reset_statements(
    generation: &Generation<'_>,
    node: StateId,
    children: &proc_macro2::Ident,
) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(node);
    if !matches!(state.kind, StateKind::Compound | StateKind::Parallel) {
        return TokenStream::new();
    }
    let variant = super::walk::variant_pattern(generation, node, super::walk::Binding::Everything);
    let payload = super::naming::payload_binding(tree, node);
    let message = format!("`{}` is active", state.name);
    let owners: Vec<StateId> = if state.kind == StateKind::Parallel {
        state.children.clone()
    } else {
        vec![node]
    };
    let replacements = owners.iter().map(|&owner| {
        let owner_state = tree.node(owner);
        let field = owner_state.children_field.as_ref().expect("children field");
        let initial =
            initial_value(tree, owner).unwrap_or_else(|_| quote! { ::core::unreachable!() });
        let retired = if owner_state.history == crate::model::HistoryKind::DeepKeep {
            let history = super::runtime::history_field(generation, owner);
            quote! { cx.runtime_mut().#history = ::core::option::Option::Some(old_substates); }
        } else if super::history::has_deep_below(generation, owner) {
            let retire =
                super::history::retire_function(owner_state.children_enum.as_ref().expect("enum"));
            quote! { #retire(old_substates, cx); }
        } else {
            quote! { drop(old_substates); }
        };
        quote! {
            let old_substates = ::core::mem::replace(&mut #payload.#field, #initial);
            #retired
        }
    });
    quote! {
        let #variant = &mut *#children else { ::core::unreachable!(#message) };
        #(#replacements)*
    }
}
