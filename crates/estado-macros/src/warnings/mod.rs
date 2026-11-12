//! Warnings of the macros (spec 6.7, ADR 0003).
//!
//! * [`emit`]: the reusable emitter (`#[deprecated]` item + `const _` call).
//! * [`size`]: W0004, needs only the state tree. Wired in [`warning_items`].
//! * [`reachability`]: W0001 and W0002, computed from the edges of the
//!   analysed handlers, wired in [`warning_items`].
//!   W0003 and W0005 also need handler information; W0010 needs typestate.

mod code;
mod emit;
mod reachability;
mod size;
#[cfg(test)]
mod tests;

use proc_macro2::TokenStream;

use crate::model::MachineModel;

pub use code::WarningCode;
#[allow(unused_imports)] // the tests build warnings by hand
pub use emit::{Warning, WarningSet, emit_warning};
#[allow(unused_imports)] // the tests build edge sources by hand
pub use reachability::EdgeSource;
pub use reachability::{edge_sources, graph_warnings};

/// The generated warnings of a machine: the hidden module and the `const _`
/// block, or nothing when no warning applies.
///
/// W0001 and W0002 come from the edges of the analysed handlers. A machine
/// with errors has no complete edge list, so they are skipped then (every
/// state would look unreachable).
pub fn warning_items(model: &MachineModel, valid: bool) -> TokenStream {
    let mut set = WarningSet::new(&model.options.allowed_warnings);
    if !set.is_allowed(WarningCode::OversizedVariant) {
        for warning in size::oversized_variants(model) {
            set.push(&warning);
        }
    }
    let graph_codes = [WarningCode::UnreachableState, WarningCode::DeadEnd];
    let wants_graph = graph_codes.iter().any(|&code| !set.is_allowed(code));
    if wants_graph && valid {
        let edges = edge_sources(model);
        for warning in graph_warnings(model, &edges) {
            if !set.is_allowed(warning.code) {
                set.push(&warning);
            }
        }
    }
    set.into_tokens()
}
