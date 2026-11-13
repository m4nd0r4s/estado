//! Code generators (bead 4.3). They only read the [`MachineModel`].
//!
//! The pieces, in the order they appear in `__estado`:
//!
//! | File | Generates |
//! | --- | --- |
//! | `context` | `Generation`: the model, the handlers by role and state, feature detection |
//! | `naming` | names of generated bindings, functions and types |
//! | `walk` | patterns, handler calls, the borrowed and owned walks down the tree |
//! | `views` | `*Fields`, `*Mut`, `*Ref`, `*Exiting` (machine module) |
//! | `runtime` | the `Runtime` struct: queues, epochs, histories, pending flags |
//! | `history` | discriminants, `retire_*`, deep-history recording |
//! | `edges` | one edge enum per handler with `Edge::apply`; the `changed` probe |
//! | `entries` | `exit_*` / `enter_*` per children enum |
//! | `tasks` | timers, invokes, epochs: `start_*` / `cancel_*` |
//! | `lookups` | `timer_event_*`, `accepts_invoke_*` |
//! | `dispatch`, `bubble` | the nested `match`, bubbling, `reenter!()` |
//! | `regions` | parallel states: the tally, region edges, `dispatch_<parallel>` |
//! | `completion`, `always` | `on_done`, `is_done`, `run_always` |
//! | `graph` | `GRAPH`, `path()`, `state_id()` |
//! | `state_ids` | the `StateId` constants of the root type |
//! | `spec` | the `Spec` impl |
//! | `stub` | the placeholder `Spec` impl of a machine with errors |

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
pub(crate) mod naming;
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
    let core_alias = crate::core_crate::core_alias_item();
    if !valid {
        let stub = stub::stub_spec(model);
        return quote! {
            #core_alias
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
    let completion = completion::completion_functions(&generation);
    let always = always::always_items(&generation);
    let task_helpers = tasks::helper_items(&generation);
    let lifecycle = tasks::lifecycle_functions(&generation);
    let lookups = lookups::lookup_functions(&generation);
    let runtime = runtime::runtime_items(&generation);
    let history_functions = history::discriminant_functions(&generation);
    let retire_functions = history::retire_functions(&generation);
    quote! {
        #core_alias
        #state_ids
        #views
        #runtime

        #[doc(hidden)]
        #[allow(
            missing_docs,
            unused_variables,
            unused_mut,
            unreachable_code,
            private_interfaces,
            private_bounds,
            clippy::too_many_arguments,
            clippy::large_enum_variant
        )]
        pub mod __estado {
            use super::*;
            use super::__estado_core::Edge as _;

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
