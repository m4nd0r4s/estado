//! Warnings of the macros (spec 6.7, ADR 0003).
//!
//! * [`emit`]: the reusable emitter (`#[deprecated]` item + `const _` call).
//! * [`size`]: W0004, needs only the state tree. Wired in [`warning_items`].
//! * [`reachability`]: W0001 and W0002, computed from the edges of the
//!   analysed handlers, wired in [`warning_items`].
//! * [`deferred_forever`]: W0003, from the `defer!()` calls and the edges.
//! * [`unhandled_timer_event`]: W0005, from the timers and the event arms.
//!
//! W0010 needs typestate (milestone M6).

mod code;
mod deferred_forever;
mod emit;
mod reachability;
mod scope;
mod size;
#[cfg(test)]
mod tests;
mod unhandled_timer_event;

use proc_macro2::TokenStream;

use crate::model::MachineModel;

pub use code::WarningCode;

/// A check that lists the warnings of one code from the analysed handlers.
type HandlerCheck = fn(&MachineModel) -> Vec<Warning>;
#[allow(unused_imports)] // the tests build warnings by hand
pub use emit::{Warning, WarningSet, emit_warning};
#[allow(unused_imports)] // the tests build edge sources by hand
pub use reachability::EdgeSource;
pub use reachability::{edge_sources, graph_warnings};

/// The generated warnings of a machine: the hidden module and the `const _`
/// block, or nothing when no warning applies.
///
/// W0001, W0002, W0003 and W0005 come from the analysed handlers. A machine
/// with errors has no complete handler list, so they are skipped then (every
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
    if valid {
        let from_handlers: [(WarningCode, HandlerCheck); 2] = [
            (
                WarningCode::DeferInDeadEnd,
                deferred_forever::deferred_forever,
            ),
            (
                WarningCode::UnhandledTimerEvent,
                unhandled_timer_event::unhandled_timer_events,
            ),
        ];
        for (code, find) in from_handlers {
            if !set.is_allowed(code) {
                for warning in find(model) {
                    set.push(&warning);
                }
            }
        }
    }
    set.into_tokens()
}
