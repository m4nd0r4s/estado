//! Decision phase of `Conn`: dispatch of the top-level states, the
//! sequential evaluation of the regions of `Connected` (spec 5.1, 5.6) and the
//! resolution of a `Next` into a transition (spec 5.3).
//!
//! # Open item 13.4: regions, sequential or "all see the event"
//!
//! The regions run in declaration order, `link` then `keepalive`, and each
//! region-internal transition is applied before the next region looks at the
//! event (spec 5.6 items 1 and 2), followed by `cx.refresh_active` (ADR 0005).
//! Where the question shows up in this scenario:
//!
//! - `FrameSent` is wanted by both regions: `link` goes `Sending -> Idle`,
//!   then `keepalive` re-enters `Pinging`. Sequential and "all see the event"
//!   give the same result, because `keepalive` does not read `link`.
//! - `on_idle` reads the sibling (`cx.in_state(AWAITING_PONG)`). `link` is
//!   evaluated first, so it sees `keepalive` as it was before the event under
//!   either semantics; the order only matters for a region that reads one
//!   evaluated *earlier* (it would see the already-applied transition).
//! - The short-circuit of item 3 applies when a region leaves `Connected`.
//!   Only `Dead` does, and `keepalive` is the last region, so no region is
//!   skipped. Declared the other way round, `link` would never see
//!   `KeepaliveDead`; nothing in `link` needs it, so it would not matter.
//! - Item 4 and 5 hide a hazard: a region that consumes an event (`Stay`,
//!   `Reenter`, `goto!`) makes the whole event "consumed", so a `Defer` from
//!   another region is dropped. If `keepalive` handled `Send` (as activity)
//!   while `link` deferred it, the frame would be lost. That is why the
//!   activity signal is `FrameSent` and not `Send`.
//!
//! Nothing here needs `#[regions(all_see_event)]`.

use estado_core::{Edge, Next, Outcome, Sink, Slot, Spec};

use super::actions::ConnCx;
use super::edges::{RegionEdge, switch_keepalive, switch_link};
use super::handlers::{
    on_awaiting_pong, on_backoff, on_connected, on_connecting, on_dead, on_disconnected, on_idle,
    on_pinging, on_sending,
};
use super::model::{Command, Conn, Connected, Event, Keepalive, Link};
use super::views::{BackoffMut, ConnectingMut, SendingMut};

/// The regions of `Connected`, in declaration order.
#[derive(Clone, Copy)]
enum Region {
    Link,
    Keepalive,
}

/// What the regions did with one event, for the verdict of spec 5.6.
#[derive(Default)]
struct Tally {
    consumed: bool,
    deferred: bool,
    transitioned: bool,
    changed: bool,
}

/// Applies `next` for a handler of the root state or of `Connected` itself.
fn resolve<Edges: Edge<Conn>, S: Sink<Command>>(
    next: Next<Conn, Edges>,
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
) -> Outcome {
    match next {
        Next::Stay => Outcome::Stayed,
        Next::Bubble | Next::Unhandled => Outcome::Unhandled(Conn::UNHANDLED),
        Next::Defer => Outcome::Deferred,
        Next::Reenter => {
            // `Backoff` and `Connecting` never return `reenter!()`.
            unreachable!("no top-level handler of Conn returns reenter!()")
        }
        Next::Goto(edge) => Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        },
        Next::__Never(_, never) => match never {},
    }
}

/// `reenter!()` of a region leaf: exit and enter the same value, so the
/// epoch moves on and the timer starts over.
fn reenter_region<S: Sink<Command>>(slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>, region: Region) {
    let Conn::Connected(connected) = slot.live_mut() else {
        unreachable!("Connected is active")
    };
    match region {
        Region::Link => {
            let same = connected.link.clone();
            switch_link(slot, cx, same);
        }
        Region::Keepalive => {
            let same = connected.keepalive;
            switch_keepalive(slot, cx, same);
        }
    }
}

/// Folds the decision of one region into the tally and applies it at once
/// (spec 5.6 item 2). Returns `true` when the transition left `Connected`:
/// the remaining regions are not evaluated (item 3).
fn absorb<Edges: RegionEdge, S: Sink<Command>>(
    next: Next<Conn, Edges>,
    region: Region,
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    tally: &mut Tally,
) -> bool {
    match next {
        Next::Stay => tally.consumed = true,
        Next::Reenter => {
            tally.consumed = true;
            tally.transitioned = true;
            reenter_region(slot, cx, region);
        }
        Next::Defer => tally.deferred = true,
        Next::Bubble | Next::Unhandled => {}
        Next::Goto(edge) => {
            tally.consumed = true;
            tally.transitioned = true;
            let leaves_parallel = edge.leaves_parallel();
            tally.changed |= edge.apply(slot, cx);
            if leaves_parallel {
                return true;
            }
            // The transition is applied: the next region must see it through
            // `cx.in_state` (ADR 0005).
            cx.refresh_active(slot.live_mut().path());
        }
        Next::__Never(_, never) => match never {},
    }
    false
}

/// Dispatch of the leaf of the `link` region.
fn evaluate_link<S: Sink<Command>>(
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
    tally: &mut Tally,
) -> bool {
    let Conn::Connected(connected) = slot.live_mut() else {
        unreachable!("Connected is active")
    };
    let (up, link, _) = connected.split();
    match link {
        Link::Idle => {
            let next = on_idle(cx, event);
            absorb(next, Region::Link, slot, cx, tally)
        }
        Link::Sending { frame } => {
            let next = on_sending(SendingMut { frame }, up, cx, event);
            absorb(next, Region::Link, slot, cx, tally)
        }
    }
}

/// Dispatch of the leaf of the `keepalive` region.
fn evaluate_keepalive<S: Sink<Command>>(
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
    tally: &mut Tally,
) -> bool {
    let Conn::Connected(Connected { keepalive, .. }) = slot.live_mut() else {
        unreachable!("Connected is active")
    };
    match keepalive {
        Keepalive::Pinging => {
            let next = on_pinging(cx, event);
            absorb(next, Region::Keepalive, slot, cx, tally)
        }
        Keepalive::AwaitingPong => {
            let next = on_awaiting_pong(cx, event);
            absorb(next, Region::Keepalive, slot, cx, tally)
        }
        Keepalive::Dead => {
            let next = on_dead(cx, event);
            absorb(next, Region::Keepalive, slot, cx, tally)
        }
    }
}

/// Spec 5.6: sequential evaluation with short-circuit.
fn dispatch_connected<S: Sink<Command>>(
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Outcome {
    let mut tally = Tally::default();
    if evaluate_link(slot, cx, event, &mut tally) {
        return Outcome::Transitioned {
            changed: tally.changed,
        };
    }
    if evaluate_keepalive(slot, cx, event, &mut tally) {
        return Outcome::Transitioned {
            changed: tally.changed,
        };
    }
    if tally.consumed {
        return if tally.transitioned {
            Outcome::Transitioned {
                changed: tally.changed,
            }
        } else {
            Outcome::Stayed
        };
    }
    if tally.deferred {
        return Outcome::Deferred;
    }
    let Conn::Connected(connected) = slot.live_mut() else {
        unreachable!("Connected is active")
    };
    let (up, _, _) = connected.split();
    let next = on_connected(up, cx, event);
    resolve(next, slot, cx)
}

/// One microstep of the decision phase: dispatch from the active leaf up.
pub fn dispatch<S: Sink<Command>>(
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Outcome {
    match slot.live_mut() {
        Conn::Disconnected => {
            let next = on_disconnected(cx, event);
            resolve(next, slot, cx)
        }
        Conn::Connecting { attempt } => {
            let next = on_connecting(ConnectingMut { attempt }, cx, event);
            resolve(next, slot, cx)
        }
        Conn::Backoff { attempt } => {
            let next = on_backoff(BackoffMut { attempt }, cx, event);
            resolve(next, slot, cx)
        }
        Conn::Connected(_) => dispatch_connected(slot, cx, event),
    }
}
