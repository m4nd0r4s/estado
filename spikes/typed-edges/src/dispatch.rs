//! The generated microstep (spec 4.1, 5.1): nested `match` on the
//! discriminants, call the handler, let its borrows end, then apply the edge
//! in the same frame.

use crate::core::{Context, Edge, Next};
use crate::model::{ActiveFields, ActiveSub, Event, Player};

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Stayed,
    Unhandled,
    Deferred,
    Transitioned,
    /// `Err` from a `Result` handler; real code routes it to `#[on_error]`.
    HandlerFailed,
}

pub fn microstep(machine: &mut Player, context: &mut Context, event: &Event) -> Outcome {
    let Player::Active(active) = machine else {
        return Outcome::Unhandled;
    };
    match &mut active.sub {
        ActiveSub::Playing { position } => {
            let up = ActiveFields {
                track: &mut active.track,
                buffer: &mut active.buffer,
            };
            let next = Player::playing(position, up, context, event);
            // Borrows of `active` end here; `next` only borrows `event`.
            match resolve(next, machine, context) {
                Some(outcome) => outcome,
                None => bubble_to_active(machine, context, event),
            }
        }
        ActiveSub::Paused { .. } => {
            let next = Player::paused(context, event);
            match resolve(next, machine, context) {
                Some(outcome) => outcome,
                None => bubble_to_active(machine, context, event),
            }
        }
    }
}

fn bubble_to_active(machine: &mut Player, context: &mut Context, event: &Event) -> Outcome {
    let Player::Active(active) = machine else {
        unreachable!("Active is active")
    };
    let up = ActiveFields {
        track: &mut active.track,
        buffer: &mut active.buffer,
    };
    match Player::active(up, context, event) {
        // Root has no handler: bubbling from `Active` is unhandled.
        Ok(next) => resolve(next, machine, context).unwrap_or(Outcome::Unhandled),
        Err(_) => Outcome::HandlerFailed,
    }
}

/// `None` means bubble to the parent.
fn resolve<E: Edge<Player>>(
    next: Next<E>,
    machine: &mut Player,
    context: &mut Context,
) -> Option<Outcome> {
    Some(match next {
        Next::Stay => Outcome::Stayed,
        Next::Bubble => return None,
        Next::Unhandled => Outcome::Unhandled,
        Next::Defer => Outcome::Deferred,
        Next::Reenter => Outcome::Transitioned,
        Next::Goto(edge) => {
            edge.apply(machine, context);
            Outcome::Transitioned
        }
    })
}
