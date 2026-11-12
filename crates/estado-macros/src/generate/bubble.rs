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
        Some(UnhandledPolicy::Ignore) => {
            quote! { ::estado_core::Outcome::Unhandled(::estado_core::UnhandledPolicy::Ignore) }
        }
        Some(UnhandledPolicy::Error) => {
            quote! { ::estado_core::Outcome::Unhandled(::estado_core::UnhandledPolicy::Error) }
        }
        Some(UnhandledPolicy::Panic) => {
            quote! { ::estado_core::Outcome::Unhandled(::estado_core::UnhandledPolicy::Panic) }
        }
    }
}

/// The event left `node` unhandled: apply its policy override, or go on to
/// the nearest ancestor that has a handler (an `Outcome` expression).
pub fn bubble_out_of(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    if let Some(policy) = tree.node(node).unhandled {
        return unhandled_outcome(generation, Some(policy));
    }
    let parent = tree.node(node).parent.unwrap_or(0);
    if parent == 0 {
        return unhandled_outcome(generation, None);
    }
    if generation.handler(HandlerRole::On, parent).is_some() {
        let function = bubble_function(tree, parent);
        return quote! { #function(slot, cx, event) };
    }
    bubble_out_of(generation, parent)
}

/// Calls `call` (the handler of `node`), resolves its `Next` and applies the
/// bubbling rules. An expression of type `Outcome`.
pub fn handle_at(generation: &Generation<'_>, node: StateId, call: TokenStream) -> TokenStream {
    let tree = generation.tree;
    let override_policy = tree.node(node).unhandled;
    let handler = generation
        .handler(HandlerRole::On, node)
        .expect("handle_at is called for a state with a handler");
    let mapped = override_policy.map(|policy| {
        let outcome = unhandled_outcome(generation, Some(policy));
        quote! { Resolution::Done(::estado_core::Outcome::Unhandled(_)) => #outcome, }
    });
    let bubble = bubble_out_of(generation, node);
    let reenter = if handler.uses.reenter {
        let function = reenter_function(tree, node);
        quote! { #function(slot, cx) }
    } else {
        quote! { ::core::unreachable!("this handler never returns `reenter!()`") }
    };
    quote! {
        {
            let next = #call;
            match resolve(next, slot, cx) {
                #mapped
                Resolution::Done(outcome) => outcome,
                Resolution::Bubble => #bubble,
                Resolution::Reenter => #reenter,
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
        let walk = borrowed_walk(generation, node.id);
        let prelude = &walk.prelude;
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
        functions.extend(quote! {
            #[doc = #documentation]
            #[inline(always)]
            pub fn #name<#sink_parameter>(
                slot: &mut ::estado_core::Slot<#root>,
                cx: &mut #cx_type,
                event: &#event,
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
        let reset = (tree.node(node).kind == StateKind::Compound).then(|| {
            let state = tree.node(node);
            let variant =
                super::walk::variant_pattern(generation, node, super::walk::Binding::Everything);
            let payload = super::naming::payload_binding(tree, node);
            let field = state.children_field.as_ref().expect("substates field");
            let initial =
                initial_value(tree, node).unwrap_or_else(|_| quote! { ::core::unreachable!() });
            let message = format!("`{}` is active", state.name);
            // The substates that leave are consumed: deep histories record them.
            let retired = if state.history == crate::model::HistoryKind::DeepKeep {
                let history = super::runtime::history_field(generation, node);
                quote! { cx.runtime_mut().#history = ::core::option::Option::Some(old_substates); }
            } else if super::history::has_deep_below(generation, node) {
                let retire =
                    super::history::retire_function(state.children_enum.as_ref().expect("enum"));
                quote! { #retire(old_substates, cx); }
            } else {
                quote! { drop(old_substates); }
            };
            quote! {
                let #variant = &mut *#children else { ::core::unreachable!(#message) };
                let old_substates = ::core::mem::replace(&mut #payload.#field, #initial);
                #retired
            }
        });
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
                    #reset
                    #enter(#children, #(#ancestors,)* cx);
                }
                ::estado_core::Outcome::Transitioned {
                    changed: <#root as ::estado_core::Spec>::state_id(slot.live_mut()) != leaf_before,
                }
            }
        });
    }
    functions
}
