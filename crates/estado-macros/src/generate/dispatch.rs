//! Dispatch (spec 5.1, ADR 0011): the nested `match` from the root to the
//! active leaf, calling the handlers from the leaf upwards in the same frame
//! and executing the chosen edge before returning.

use proc_macro2::TokenStream;
use quote::quote;

use super::bubble::{bubble_functions, bubble_out_of, handle_at, reenter_functions};
use super::context::Generation;
use super::naming::children_binding;
use super::walk::{Binding, FieldsAccess, call_handler, split_payload, variant_pattern};
use crate::model::{HandlerRole, StateId, StateKind};

/// `Resolution`, `resolve`, the bubbling functions and `microstep`.
pub fn dispatch_items(generation: &Generation<'_>) -> TokenStream {
    let resolution = resolution_items(generation);
    let bubbles = bubble_functions(generation);
    let reenters = reenter_functions(generation);
    let microstep = microstep_function(generation);
    let regions = super::regions::region_items(generation);
    quote! { #resolution #bubbles #reenters #regions #microstep }
}

/// What the dispatch does after a handler returned its `Next`.
fn resolution_items(generation: &Generation<'_>) -> TokenStream {
    let root = generation.root;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    // A machine without `reenter!()` has no such variant: no handler site
    // needs a panic for it.
    let (reenter_variant, reenter_arm) = if generation.uses_reenter() {
        (
            quote! {
                /// Exit and enter the handler's own state.
                Reenter,
            },
            quote! { __estado_core::Next::Reenter => Resolution::Reenter, },
        )
    } else {
        (
            TokenStream::new(),
            quote! { __estado_core::Next::Reenter => ::core::unreachable!(), },
        )
    };
    quote! {
        /// What the dispatch does after a handler returned.
        pub enum Resolution {
            /// The microstep is over.
            Done(__estado_core::Outcome),
            /// Delegate to the parent.
            Bubble,
            #reenter_variant
        }

        /// Turns the `Next` of a handler into a [`Resolution`], executing the edge.
        #[inline(always)]
        pub fn resolve<Edges: __estado_core::Edge<#root>, #sink_parameter>(
            next: __estado_core::Next<#root, Edges>,
            slot: &mut __estado_core::Slot<#root>,
            cx: &mut #cx_type,
        ) -> Resolution {
            match next {
                __estado_core::Next::Stay => Resolution::Done(__estado_core::Outcome::Stayed),
                __estado_core::Next::Bubble => Resolution::Bubble,
                __estado_core::Next::Unhandled => Resolution::Done(__estado_core::Outcome::Unhandled(
                    <#root as __estado_core::Spec>::UNHANDLED,
                )),
                __estado_core::Next::Defer => Resolution::Done(__estado_core::Outcome::Deferred),
                #reenter_arm
                __estado_core::Next::Goto(edge) => Resolution::Done(__estado_core::Outcome::Transitioned {
                    changed: edge.apply(slot, cx),
                }),
                __estado_core::Next::__Never(_, never) => match never {},
            }
        }
    }
}

/// The nested `match` over the active configuration.
fn microstep_function(generation: &Generation<'_>) -> TokenStream {
    let root = generation.root;
    let event = &generation.model.options.event;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let arms = children_arms(generation, 0);
    quote! {
        /// One microstep: the nested `match` from the root to the active leaf.
        #[inline(always)]
        pub fn microstep<#sink_parameter>(
            slot: &mut __estado_core::Slot<#root>,
            cx: &mut #cx_type,
            event: &#event,
        ) -> __estado_core::Outcome {
            match slot.live_mut() {
                #arms
            }
        }
    }
}

/// The arms of a `match` over the children enum of `owner`.
pub fn children_arms(generation: &Generation<'_>, owner: StateId) -> TokenStream {
    let tree = generation.tree;
    let arms = tree.node(owner).children.iter().map(|&child| {
        let state = tree.node(child);
        if state.kind == StateKind::Compound {
            return compound_arm(generation, child);
        }
        if state.kind == StateKind::Parallel {
            let pattern = variant_pattern(generation, child, Binding::Nothing);
            let function = super::regions::dispatch_function(generation, child);
            return quote! { #pattern => #function(slot, cx, event), };
        }
        let handler = generation.handler(HandlerRole::On, child);
        let binding = if handler.is_some() {
            Binding::Everything
        } else {
            Binding::Nothing
        };
        let pattern = variant_pattern(generation, child, binding);
        let body = match handler {
            Some(handler) => handle_at(
                generation,
                child,
                call_handler(generation, handler, FieldsAccess::Binding, None),
            ),
            None => bubble_out_of(generation, child),
        };
        quote! { #pattern => #body, }
    });
    quote! { #(#arms)* }
}

/// A compound variant: split its borrow and match on its substates.
fn compound_arm(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    let pattern = variant_pattern(generation, node, Binding::Everything);
    let substates = children_binding(tree, node);
    let split = split_payload(generation, node);
    let arms = children_arms(generation, node);
    quote! {
        #pattern => {
            #split
            match #substates {
                #arms
            }
        }
    }
}
