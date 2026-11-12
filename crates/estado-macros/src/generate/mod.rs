//! Code generators (bead 4.3). They only read the [`MachineModel`].
//!
//! The pieces, in the order they appear in `__estado`:
//!
//! | File | Generates |
//! | --- | --- |
//! | `views` | `*Fields`, `*Mut`, `*Exiting` (machine module) |
//! | `edges` | one edge enum per handler with `Edge::apply` |
//! | `entries` | `exit_*` / `enter_*` per children enum |
//! | `dispatch`, `bubble` | the nested `match`, bubbling, `reenter!()` |
//! | `regions` | parallel states: the tally, region edges, `dispatch_<parallel>` |
//! | `tasks` | timers, invokes, epochs; `timer_event`, `accepts_invoke` |
//! | `completion`, `always` | `on_done`, `is_done`, `run_always` |
//! | `graph` | `GRAPH`, `path()`, `state_id()` |
//! | `spec` | the `Spec` impl |

mod always;
mod bubble;
mod completion;
mod context;
mod dispatch;
mod edges;
mod entries;
mod graph;
mod history;
mod lookups;
mod naming;
mod regions;
mod runtime;
mod spec;
mod state_ids;
mod stub;
mod tasks;
mod views;
mod walk;

use proc_macro2::TokenStream;
use quote::quote;

use crate::model::MachineModel;
use context::Generation;

/// Everything generated for a machine, in the machine module. With
/// `valid == false` (the machine has errors) only the state ids are emitted,
/// so the user's own code keeps resolving them.
pub fn generated_items(model: &MachineModel, valid: bool) -> TokenStream {
    let state_ids = state_ids::state_id_constants(model);
    if !valid {
        let stub = stub::stub_spec(model);
        return quote! {
            #state_ids
            #stub

            #[doc(hidden)]
            pub mod __estado {}
        };
    }
    let generation = Generation::new(model);
    let views = views::view_items(&generation);
    let edges = edges::edge_items(&generation);
    let entries = entries::entry_exit_functions(&generation);
    let dispatch = dispatch::dispatch_items(&generation);
    let graph = graph::graph_items(&generation);
    let spec = spec::spec_impl(&generation);
    let completion = completion::has_completion(&generation)
        .then(|| completion::completion_function(&generation));
    let always = always::always_items(&generation);
    let task_helpers = tasks::helper_items(&generation);
    let lifecycle = tasks::lifecycle_functions(&generation);
    let lookups = lookups::lookup_functions(&generation);
    let runtime = runtime::runtime_items(&generation);
    let history_functions = history::discriminant_functions(&generation);
    let retire_functions = history::retire_functions(&generation);
    quote! {
        #state_ids
        #views

        #[doc(hidden)]
        #[allow(
            missing_docs,
            unused_variables,
            unused_mut,
            unreachable_code,
            private_interfaces,
            clippy::too_many_arguments,
            clippy::large_enum_variant
        )]
        pub mod __estado {
            use super::*;
            use ::estado_core::Edge as _;

            #runtime
            #task_helpers
            #history_functions
            #retire_functions
            #edges
            #entries
            #lifecycle
            #lookups
            #dispatch
            #completion
            #always
            #graph
            #spec
        }
    }
}
