//! Dispatch and the `Spec` impl: a nested `match` from the active leaf up
//! (spec 5.1), then the chosen edge applies in the same frame (spec 4.1).

use estado_core::{
    Edge, Graph, InlineEffects, Next, NoRuntime, Outcome, Sink, Slot, Spec, StateId, StatePath,
};

use super::actions::{PlayerCx, enter_root_state};
use super::graph::GRAPH;
use super::handlers::{on_active, on_failed, on_idle, on_paused, on_playing};
use super::model::{ActiveSub, Command, Event, Player};
use super::views::{FailedMut, PausedMut, PlayingMut};

/// What the dispatch does after a handler returned.
enum Resolution {
    Done(Outcome),
    Bubble,
}

fn resolve<E: Edge<Player>, S: Sink<Command>>(
    next: Next<Player, E>,
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
) -> Resolution {
    match next {
        Next::Stay => Resolution::Done(Outcome::Stayed),
        Next::Bubble => Resolution::Bubble,
        Next::Unhandled => Resolution::Done(Outcome::Unhandled(Player::UNHANDLED)),
        Next::Defer => Resolution::Done(Outcome::Deferred),
        Next::Reenter => unreachable!("no handler of Player returns reenter!()"),
        Next::Goto(edge) => Resolution::Done(Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        }),
        Next::__Never(_, never) => match never {},
    }
}

/// Outcome of a top-level state: bubbling past the root is unhandled.
fn resolve_top_level<E: Edge<Player>, S: Sink<Command>>(
    next: Next<Player, E>,
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
) -> Outcome {
    match resolve(next, slot, cx) {
        Resolution::Done(outcome) => outcome,
        Resolution::Bubble => Outcome::Unhandled(Player::UNHANDLED),
    }
}

fn bubble_to_active<S: Sink<Command>>(
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
    event: &Event,
) -> Outcome {
    let Player::Active(active) = slot.live_mut() else {
        unreachable!("Active is active")
    };
    let (up, _) = active.split();
    let next = on_active(up, cx, event);
    resolve_top_level(next, slot, cx)
}

fn dispatch_in_active<S: Sink<Command>>(
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
    event: &Event,
) -> Outcome {
    let Player::Active(active) = slot.live_mut() else {
        unreachable!("Active is active")
    };
    // The split borrow: `up` and `sub` are disjoint fields of `active`.
    let (up, sub) = active.split();
    let resolution = match sub {
        ActiveSub::Playing { pos } => {
            let next = on_playing(PlayingMut { pos }, up, cx, event);
            resolve(next, slot, cx)
        }
        ActiveSub::Paused { pos } => {
            let next = on_paused(PausedMut { pos }, up, cx, event);
            resolve(next, slot, cx)
        }
    };
    match resolution {
        Resolution::Done(outcome) => outcome,
        Resolution::Bubble => bubble_to_active(slot, cx, event),
    }
}

impl Spec for Player {
    type Event = Event;
    type Effect = Command;
    type Ctx = Vec<String>;
    type Output = ();
    type Runtime = NoRuntime<Event>;
    type Effects = InlineEffects<Command, 4>;

    const NAME: &'static str = "Player";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Self::Idle
    }

    fn state_id(&self) -> StateId {
        self.active_path().leaf().expect("never empty")
    }

    fn path(&self) -> StatePath {
        self.active_path()
    }

    fn enter_initial<S: Sink<Command>>(&mut self, cx: &mut PlayerCx<'_, S>) {
        enter_root_state(self, cx);
    }

    fn microstep<S: Sink<Command>>(
        slot: &mut Slot<Self>,
        cx: &mut PlayerCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        match slot.live_mut() {
            Self::Idle => {
                let next = on_idle(cx, event);
                resolve_top_level(next, slot, cx)
            }
            Self::Failed { reason, buffer } => {
                let next = on_failed(FailedMut { reason, buffer }, cx, event);
                resolve_top_level(next, slot, cx)
            }
            Self::Active(_) => dispatch_in_active(slot, cx, event),
        }
    }
}
