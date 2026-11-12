//! Decision phase of `Conn`: dispatch from the leaf up, the sequential
//! evaluation of the regions of `Connected` (spec 5.1, 5.6), and the
//! resolution of a `Next` into a transition (spec 5.3).

use estado_core::{Edge, Next, Outcome, Sink, Slot, Spec};

use super::edges::{Goto, reenter_all, reenter_leaf};
use super::graph::Leaf;
use super::{Conn, ConnCx, Connected, ConnectedFields, Event, Keepalive, Link, handlers};

/// The regions of `Connected`, in declaration order.
#[derive(Clone, Copy)]
enum Region {
    Link,
    Keepalive,
}

impl Conn {
    fn connected_mut(&mut self) -> &mut Connected {
        match self {
            Self::Connected(connected) => connected,
            _ => unreachable!("only called while Connected is active"),
        }
    }
}

/// Spec 5.4 item "microstep" for one decision: applies `next`.
pub fn resolve<Edges: Edge<Conn>, S: Sink<()>>(
    next: Next<Conn, Edges>,
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
) -> Outcome {
    match next {
        Next::Stay => Outcome::Stayed,
        Next::Bubble | Next::Unhandled => Outcome::Unhandled(Conn::UNHANDLED),
        Next::Defer => Outcome::Deferred,
        Next::Reenter => {
            reenter_all(slot.live_mut(), cx);
            Outcome::Transitioned { changed: false }
        }
        Next::Goto(edge) => Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        },
        Next::__Never(_, never) => match never {},
    }
}

/// Dispatch of the leaf of one region (spec 5.6 item 1). A region has no
/// handler of its own, so `Bubble` means the region did not handle it.
fn dispatch_region<S: Sink<()>>(
    connected: &Connected,
    region: Region,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, Goto> {
    match region {
        Region::Link => match connected.link {
            Link::Opening => handlers::opening(cx, event),
            Link::Open => handlers::open(cx, event),
            Link::Finished => Next::Bubble,
        },
        Region::Keepalive => match connected.keepalive {
            Keepalive::Pinging => handlers::pinging(cx, event),
            Keepalive::AwaitingPong => handlers::awaiting_pong(cx, event),
            Keepalive::Dead => handlers::dead(cx, event),
            Keepalive::Stopped => Next::Bubble,
        },
    }
}

/// Spec 5.6: sequential evaluation with short-circuit.
pub fn dispatch_connected<S: Sink<()>>(
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Outcome {
    let mut consumed = false;
    let mut deferred = false;
    let mut transitioned = false;
    let mut changed = false;
    for region in [Region::Link, Region::Keepalive] {
        let next = dispatch_region(slot.live_mut().connected_mut(), region, cx, event);
        match next {
            Next::Stay => consumed = true,
            Next::Reenter => {
                consumed = true;
                transitioned = true;
                let connected = slot.live_mut().connected_mut();
                reenter_leaf(
                    match region {
                        Region::Link => connected.link.state_id(),
                        Region::Keepalive => connected.keepalive.state_id(),
                    },
                    cx,
                );
            }
            Next::Defer => deferred = true,
            Next::Bubble | Next::Unhandled => {}
            Next::Goto(edge) => {
                consumed = true;
                transitioned = true;
                let leaves_parallel = edge.leaves_parallel();
                changed |= edge.apply(slot, cx);
                if leaves_parallel {
                    return Outcome::Transitioned { changed };
                }
                // Item 2: the transition is already applied; the next region
                // must see the new configuration through `in_state`.
                cx.refresh_active(slot.live_mut().path());
            }
            Next::__Never(_, never) => match never {},
        }
    }
    if consumed {
        return if transitioned {
            Outcome::Transitioned { changed }
        } else {
            Outcome::Stayed
        };
    }
    if deferred {
        return Outcome::Deferred;
    }
    let Connected {
        session,
        frames,
        resumed_after,
        ..
    } = slot.live_mut().connected_mut();
    let up = ConnectedFields {
        session,
        frames,
        resumed_after,
    };
    let next = handlers::connected(up, cx, event);
    resolve(next, slot, cx)
}
