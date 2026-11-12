//! Code generators. They only read the [`MachineModel`].
//!
//! 4.1 emits the state id constants and an empty `__estado` module; the
//! `Spec` implementation, edges and views arrive with 4.2 and 4.3.

mod state_ids;

use proc_macro2::TokenStream;
use quote::quote;

use crate::model::MachineModel;

/// Everything generated for a machine, in the machine module.
pub fn generated_items(model: &MachineModel) -> TokenStream {
    let state_ids = state_ids::state_id_constants(model);
    quote! {
        #state_ids

        #[doc(hidden)]
        pub mod __estado {}
    }
}
