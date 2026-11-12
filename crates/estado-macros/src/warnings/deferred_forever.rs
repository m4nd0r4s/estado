//! W0003: a `defer!()` in a state from which the machine can never leave.
//!
//! A deferred event is redelivered only when the active configuration
//! changes (spec 5.4, 5.5). If no transition can happen while the deferring
//! state is active, the event waits in the queue until it overflows.
//!
//! "A transition can happen" is read from the handlers: a `goto!` (a `#[on]`
//! reaction, a timer's event, `#[always]` or `#[on_done]` all use one) or a
//! `reenter!()` in the handler of any state in play (see [`states_in_play`]):
//! the state itself, its ancestors, its descendants, and, inside a parallel
//! state, its sibling regions. A defer in a state whose ancestor can leave is
//! therefore fine.

use std::collections::HashSet;

use super::code::WarningCode;
use super::emit::Warning;
use super::scope::states_in_play;
use crate::model::{MachineModel, StateId};

/// One warning per `defer!()` call in a state that cannot be left.
///
/// Call this only for a machine without errors: with a handler missing from
/// the model every state would look stuck.
pub fn deferred_forever(model: &MachineModel) -> Vec<Warning> {
    let tree = &model.tree;
    let states_with_exits: HashSet<StateId> = model
        .analysed_handlers
        .iter()
        .filter(|handler| !handler.gotos.is_empty() || handler.uses.reenter)
        .map(|handler| handler.state)
        .collect();
    let mut warnings = Vec::new();
    for handler in &model.analysed_handlers {
        if handler.defer_calls.is_empty() {
            continue;
        }
        let can_leave = states_in_play(tree, handler.state)
            .iter()
            .any(|state| states_with_exits.contains(state));
        if can_leave {
            continue;
        }
        let name = &tree.node(handler.state).name;
        for (index, &span) in handler.defer_calls.iter().enumerate() {
            warnings.push(Warning {
                code: WarningCode::DeferInDeadEnd,
                span,
                subject: format!("{}_{index}", crate::naming::plain_name(name)),
                message: format!(
                    "`defer!()` in `{name}`, which has no outgoing edges: the configuration \
                     never changes, so the deferred event is never redelivered"
                ),
                condition: None,
            });
        }
    }
    warnings
}
