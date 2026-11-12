//! Warnings of the macros (spec 6.7, ADR 0003).
//!
//! * [`emit`]: the reusable emitter (`#[deprecated]` item + `const _` call).
//! * [`size`]: W0004, needs only the state tree. Wired in [`warning_items`].
//! * [`reachability`]: W0001 and W0002, need the edges. The analysis is done
//!   and tested; the call site waits for the edges of the handler rewrite.
//!   W0003 and W0005 also need handler information; W0010 needs typestate.

mod code;
mod emit;
#[allow(dead_code)] // the call site waits for the edges of the handler rewrite
mod reachability;
mod size;
#[cfg(test)]
mod tests;

use proc_macro2::TokenStream;

use crate::model::MachineModel;

pub use code::WarningCode;
#[allow(unused_imports)] // used by the W0001/W0002 call site once edges exist
pub use emit::{Warning, WarningSet, emit_warning};
#[allow(unused_imports)] // used by the W0001/W0002 call site once edges exist
pub use reachability::{EdgeSource, graph_warnings};

/// The generated warnings of a machine: the hidden module and the `const _`
/// block, or nothing when no warning applies.
///
/// TODO(edges): when `model.analysed_handlers` is filled, build an
/// [`EdgeSource`] per handler from `HandlerModel::gotos` (`goto_dyn!` sets
/// `has_dynamic_target`) and push every warning of [`graph_warnings`]. Do
/// not call it earlier: without the edges every state looks unreachable.
pub fn warning_items(model: &MachineModel) -> TokenStream {
    let mut set = WarningSet::new(&model.options.allowed_warnings);
    if !set.is_allowed(WarningCode::OversizedVariant) {
        for warning in size::oversized_variants(model) {
            set.push(&warning);
        }
    }
    set.into_tokens()
}
