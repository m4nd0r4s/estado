//! `Root::WORST_CASE_ACTIONS` (spec 8.3): the most entry and exit calls one
//! transition of the machine can make, as a constant for WCET analysis.

use proc_macro2::TokenStream;
use quote::quote;

use super::context::Generation;
use super::naming::state_constant;
use super::state_ids::root_generics;
use crate::model::HandlerRole;

/// The `impl Root { pub const WORST_CASE_ACTIONS }` block.
///
/// The constant is `Graph::worst_case_actions` (ADR 0019) of the static
/// graph, raised to the cost of every `reenter!()`: a re-entry is a
/// transition to the handler's own state that the graph does not list as an
/// edge. It is computed by the compiler when the constant is used, so a
/// machine that never reads it pays nothing.
pub fn worst_case_constant(generation: &Generation<'_>) -> TokenStream {
    let Some(generics) = root_generics(generation.tree) else {
        return TokenStream::new();
    };
    let root = generation.root;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let mut reentering_states = Vec::new();
    for handler in &generation.model.analysed_handlers {
        if handler.role == HandlerRole::On
            && handler.uses.reenter
            && !reentering_states.contains(&handler.state)
        {
            reentering_states.push(handler.state);
        }
    }
    let reenters = reentering_states.iter().map(|&state| {
        let id = state_constant(generation, state);
        quote! {
            let reentry = graph.reenter_actions(#id);
            if reentry > worst {
                worst = reentry;
            }
        }
    });
    quote! {
        impl #impl_generics #root #type_generics #where_clause {
            /// The largest number of `#[entry]` and `#[exit]` calls one
            /// transition of this machine can make (spec 8.3): an upper
            /// bound over the graph and the `reenter!()` sites, for WCET
            /// analysis. See `Graph::worst_case_actions`.
            pub const WORST_CASE_ACTIONS: usize = {
                let graph = <Self as __estado_core::Spec>::GRAPH;
                let mut worst = graph.worst_case_actions();
                #(#reenters)*
                worst
            };
        }
    }
}
