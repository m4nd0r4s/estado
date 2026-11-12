//! Parallel states (spec 5.6, ADR 0005 and 0008): the regions are evaluated
//! one after the other in declaration order, a transition internal to a
//! region is applied at once, a transition out of the parallel state stops
//! the evaluation.
//!
//! | Generated | Does |
//! | --- | --- |
//! | `RegionTally` | what the regions did with the event so far |
//! | `RegionEdge` | edges of region handlers know whether they leave the parallel state |
//! | `resolve_region` | like `resolve`, for the handler of a state inside a region |
//! | `evaluate_<region>` | the nested `match` of one region, from the slot to its leaf |
//! | `dispatch_<parallel>` | the regions in order, then the verdict of spec 5.6 |

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::bubble::{bubble_out_of, handle_at};
use super::context::Generation;
use super::dispatch::children_arms;
use super::naming::fields_binding;
use super::walk::{borrowed_walk, call_handler};
use crate::model::{HandlerRole, StateId, StateKind};
use crate::naming::snake_case_name;

/// The parallel states of the machine, in tree order.
pub fn parallel_states(generation: &Generation<'_>) -> Vec<StateId> {
    generation
        .tree
        .nodes
        .iter()
        .filter(|node| node.kind == StateKind::Parallel)
        .map(|node| node.id)
        .collect()
}

/// `evaluate_link`.
pub fn evaluate_function(generation: &Generation<'_>, region: StateId) -> proc_macro2::Ident {
    let name = &generation.tree.node(region).name;
    format_ident!("evaluate_{}", snake_case_name(name))
}

/// `dispatch_connected`.
pub fn dispatch_function(generation: &Generation<'_>, parallel: StateId) -> proc_macro2::Ident {
    let name = &generation.tree.node(parallel).name;
    format_ident!("dispatch_{}", snake_case_name(name))
}

/// `region_leaf_link`: the id of the active leaf of a region.
pub fn region_leaf_function(generation: &Generation<'_>, region: StateId) -> proc_macro2::Ident {
    let name = &generation.tree.node(region).name;
    format_ident!("region_leaf_{}", snake_case_name(name))
}

/// Everything for the parallel states; nothing when there are none.
pub fn region_items(generation: &Generation<'_>) -> TokenStream {
    let parallels = parallel_states(generation);
    if parallels.is_empty() {
        return TokenStream::new();
    }
    let support = support_items(generation);
    let functions = parallels.iter().map(|&parallel| {
        let tree = generation.tree;
        let evaluations = tree
            .node(parallel)
            .children
            .iter()
            .map(|&region| evaluate_region(generation, region));
        let leaves = tree
            .node(parallel)
            .children
            .iter()
            .map(|&region| region_leaf(generation, region));
        let dispatch = dispatch_parallel(generation, parallel);
        quote! { #(#evaluations)* #(#leaves)* #dispatch }
    });
    quote! { #support #(#functions)* }
}

fn support_items(generation: &Generation<'_>) -> TokenStream {
    let root = generation.root;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    quote! {
        /// What the regions of a parallel state did with one event (spec 5.6).
        #[derive(Default)]
        pub struct RegionTally {
            /// A region returned `stay!`, `goto!` or `reenter!`.
            pub consumed: bool,
            /// A region returned `defer!`.
            pub deferred: bool,
            /// A region transitioned.
            pub transitioned: bool,
            /// A transition changed the active configuration.
            pub changed: bool,
            /// A transition left the parallel state.
            pub left: bool,
            /// A state with `#[unhandled]` inside a region stopped the event.
            pub stopped: ::core::option::Option<::estado_core::UnhandledPolicy>,
        }

        impl RegionTally {
            /// Folds the outcome of one region into the tally.
            #[inline(always)]
            pub fn absorb(&mut self, outcome: ::estado_core::Outcome) {
                match outcome {
                    ::estado_core::Outcome::Stayed => self.consumed = true,
                    ::estado_core::Outcome::Deferred => self.deferred = true,
                    ::estado_core::Outcome::Unhandled(_) => {}
                    ::estado_core::Outcome::Transitioned { changed } => {
                        self.consumed = true;
                        self.transitioned = true;
                        self.changed |= changed;
                    }
                }
            }

            /// The event left a state with an `#[unhandled]` override: the
            /// policy applies there, the parallel state's own handler does
            /// not see the event (spec 5.1 item 4).
            #[inline(always)]
            pub fn stop(&mut self, policy: ::estado_core::UnhandledPolicy) -> ::estado_core::Outcome {
                self.stopped = ::core::option::Option::Some(policy);
                ::estado_core::Outcome::Unhandled(policy)
            }
        }

        /// An edge of a handler inside a region: it knows whether it leaves
        /// the parallel state, which stops the evaluation of the regions
        /// that are left (spec 5.6 item 3).
        pub trait RegionEdge: ::estado_core::Edge<#root> {
            /// Whether applying this edge exits the parallel state.
            fn leaves_parallel(&self) -> bool;
        }

        impl RegionEdge for ::estado_core::NoEdge {
            fn leaves_parallel(&self) -> bool {
                match *self {}
            }
        }

        /// Like `resolve`, for the handler of a state inside a region.
        #[inline(always)]
        pub fn resolve_region<Edges: RegionEdge, #sink_parameter>(
            next: ::estado_core::Next<#root, Edges>,
            slot: &mut ::estado_core::Slot<#root>,
            cx: &mut #cx_type,
            tally: &mut RegionTally,
        ) -> Resolution {
            match next {
                ::estado_core::Next::Goto(edge) => {
                    let leaves_parallel = edge.leaves_parallel();
                    let changed = ::estado_core::Edge::apply(edge, slot, cx);
                    if leaves_parallel {
                        tally.left = true;
                    }
                    Resolution::Done(::estado_core::Outcome::Transitioned { changed })
                }
                other => resolve(other, slot, cx),
            }
        }
    }
}

/// The id of the active leaf of one region, for `changed` of a transition
/// inside it.
fn region_leaf(generation: &Generation<'_>, region: StateId) -> TokenStream {
    let root = generation.root;
    let name = region_leaf_function(generation, region);
    let walk = borrowed_walk(generation, region);
    let prelude = &walk.prelude;
    let children = &walk.children;
    let arms = super::graph::leaf_arms(generation, region);
    quote! {
        /// Id of the active leaf of the region.
        pub fn #name(slot: &mut ::estado_core::Slot<#root>) -> ::estado_core::StateId {
            #prelude
            match &*#children {
                #arms
            }
        }
    }
}

/// The evaluation of one region: walk down to it, match its active leaf.
fn evaluate_region(generation: &Generation<'_>, region: StateId) -> TokenStream {
    let root = generation.root;
    let event = &generation.model.options.event;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let name = evaluate_function(generation, region);
    let walk = borrowed_walk(generation, region);
    let prelude = &walk.prelude;
    let children = &walk.children;
    let arms = children_arms(generation, region);
    let documentation = format!(
        "Dispatch of the active state of the region `{}`.",
        generation.tree.node(region).name
    );
    quote! {
        #[doc = #documentation]
        #[inline]
        pub fn #name<#sink_parameter>(
            slot: &mut ::estado_core::Slot<#root>,
            cx: &mut #cx_type,
            event: &#event,
            tally: &mut RegionTally,
        ) -> ::estado_core::Outcome {
            #prelude
            match #children {
                #arms
            }
        }
    }
}

/// Spec 5.6: the regions in declaration order, with the verdict.
fn dispatch_parallel(generation: &Generation<'_>, parallel: StateId) -> TokenStream {
    let tree = generation.tree;
    let root = generation.root;
    let event = &generation.model.options.event;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let name = dispatch_function(generation, parallel);
    let regions = &tree.node(parallel).children;
    let tracks_active = generation.tracks_active();
    let steps = regions.iter().enumerate().map(|(index, &region)| {
        let evaluate = evaluate_function(generation, region);
        let is_last = index + 1 == regions.len();
        // The last region has nobody left to tell (ADR 0008).
        let refresh = (tracks_active && !is_last).then(|| {
            quote! {
                if ::core::matches!(outcome, ::estado_core::Outcome::Transitioned { .. }) {
                    cx.refresh_active(::estado_core::Spec::path(slot.live_mut()));
                }
            }
        });
        quote! {
            let outcome = #evaluate(slot, cx, event, &mut tally);
            tally.absorb(outcome);
            if tally.left {
                return ::estado_core::Outcome::Transitioned { changed: tally.changed };
            }
            #refresh
        }
    });
    let own = generation.handler(HandlerRole::On, parallel).map_or_else(
        || bubble_out_of(generation, parallel),
        |handler| {
            let walk = borrowed_walk(generation, parallel);
            let prelude = &walk.prelude;
            let fields = |ancestor: StateId| {
                let binding = fields_binding(tree, ancestor);
                quote! { #binding }
            };
            let body = handle_at(
                generation,
                parallel,
                call_handler(generation, handler, &fields, None),
            );
            quote! { { #prelude #body } }
        },
    );
    let documentation = format!(
        "Spec 5.6 for `{}`: sequential evaluation of the regions with short-circuit.",
        tree.node(parallel).name
    );
    quote! {
        #[doc = #documentation]
        #[inline(always)]
        pub fn #name<#sink_parameter>(
            slot: &mut ::estado_core::Slot<#root>,
            cx: &mut #cx_type,
            event: &#event,
        ) -> ::estado_core::Outcome {
            let mut tally = RegionTally::default();
            #(#steps)*
            if tally.consumed {
                return if tally.transitioned {
                    ::estado_core::Outcome::Transitioned { changed: tally.changed }
                } else {
                    ::estado_core::Outcome::Stayed
                };
            }
            if tally.deferred {
                return ::estado_core::Outcome::Deferred;
            }
            if let ::core::option::Option::Some(policy) = tally.stopped {
                return ::estado_core::Outcome::Unhandled(policy);
            }
            #own
        }
    }
}
