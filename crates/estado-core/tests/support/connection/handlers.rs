//! The handlers the user would write inside `#[handlers]`: pure decisions
//! returning `Next`, plus the logging every test asserts on.

use estado_core::{Edge, Next, Sink};

use super::edges::{ConnectedToClosed, Goto, IdleToConnected, note};
use super::{
    Conn, ConnCx, Connected, ConnectedFields, Event, Idle, IdleFields, Keepalive, Link, Receipt,
};

type Decision = Next<Conn, Goto>;

/// `#[on(Waiting)]`
pub fn waiting<S: Sink<()>>(up: IdleFields<'_>, cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("Waiting handles {event:?}"));
    match event {
        Event::Ring(session) => Next::Goto(Goto::IdleSubTo(super::IdleSub::Ready(*session))),
        Event::Retry => {
            *up.attempts += 1;
            Next::Stay
        }
        _ => Next::Bubble,
    }
}

/// `#[on(Opening)]`, a leaf of the `link` region.
pub fn opening<S: Sink<()>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("Opening handles {event:?}"));
    match event {
        Event::LinkUp | Event::Sync => Next::Goto(Goto::LinkTo(Link::Open)),
        Event::Close | Event::CloseLink => Next::Goto(Goto::LinkTo(Link::Finished)),
        Event::Hangup => Next::Stay,
        Event::Hold => Next::Defer,
        _ => Next::Bubble,
    }
}

/// `#[on(Open)]`
pub fn open<S: Sink<()>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("Open handles {event:?}"));
    match event {
        Event::Sync | Event::Hold => Next::Stay,
        Event::Refresh => Next::Reenter,
        Event::Hangup => Next::Goto(Goto::Idle),
        Event::Close | Event::CloseLink => Next::Goto(Goto::LinkTo(Link::Finished)),
        _ => Next::Bubble,
    }
}

/// `#[on(Pinging)]`, a leaf of the `keepalive` region. Reads the sibling
/// region through `cx.in_state`.
pub fn pinging<S: Sink<()>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("Pinging handles {event:?}"));
    match event {
        Event::Timeout => Next::Goto(Goto::KeepaliveTo(Keepalive::AwaitingPong)),
        Event::Sync if cx.in_state(Conn::OPEN) => {
            note(cx, "keepalive sees link Open".into());
            Next::Goto(Goto::KeepaliveTo(Keepalive::AwaitingPong))
        }
        Event::Sync => {
            note(cx, "keepalive sees link not Open".into());
            Next::Stay
        }
        Event::Hangup => Next::Goto(Goto::Idle),
        Event::Close => Next::Goto(Goto::KeepaliveTo(Keepalive::Stopped)),
        Event::Hold => Next::Defer,
        _ => Next::Bubble,
    }
}

/// `#[on(AwaitingPong)]`
pub fn awaiting_pong<S: Sink<()>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("AwaitingPong handles {event:?}"));
    match event {
        Event::Timeout => Next::Goto(Goto::KeepaliveTo(Keepalive::Dead)),
        Event::Close => Next::Goto(Goto::KeepaliveTo(Keepalive::Stopped)),
        _ => Next::Bubble,
    }
}

/// `#[on(Dead)]`: leaves the parallel state.
pub fn dead<S: Sink<()>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("Dead handles {event:?}"));
    match event {
        Event::Reset => Next::Goto(Goto::Idle),
        _ => Next::Bubble,
    }
}

/// `#[on(Connected)]`, the handler of the parallel state itself: reached
/// only when no region consumed or deferred the event.
pub fn connected<S: Sink<()>>(
    up: ConnectedFields<'_>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Decision {
    note(cx, format!("Connected handles {event:?}"));
    match event {
        Event::Poke => {
            *up.frames += 1;
            Next::Stay
        }
        Event::Abort => Next::Goto(Goto::Idle),
        Event::Drain => Next::Goto(Goto::Draining),
        _ => Next::Bubble,
    }
}

/// `#[on(Flushing)]`
pub fn flushing<S: Sink<()>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("Flushing handles {event:?}"));
    match event {
        Event::Flush => Next::Goto(Goto::DrainSubTo(super::DrainSub::Flushed)),
        _ => Next::Bubble,
    }
}

/// `#[on(Draining)]`: `Draining` has no `#[on_done]`, so completing it does
/// nothing; this handler is how it is left.
pub fn draining<S: Sink<()>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Decision {
    note(cx, format!("Draining handles {event:?}"));
    match event {
        Event::Reset => Next::Goto(Goto::Idle),
        _ => Next::Bubble,
    }
}

/// `#[on_done(Idle)]`: receives the fields of `Idle` and the payload of the
/// final substate `Ready` as the output. The constructor copies what it needs
/// into locals first (ADR 0001): it may not capture the state's data.
pub fn on_done_idle<S: Sink<()>>(
    up: IdleFields<'_>,
    output: &u32,
    cx: &mut ConnCx<'_, S>,
) -> Next<Conn, impl Edge<Conn> + use<S>> {
    note(
        cx,
        format!("on_done Idle (output {output}, attempts {})", up.attempts),
    );
    let session = *output;
    Next::Goto(IdleToConnected(move |old: Idle| Connected {
        session,
        frames: 0,
        resumed_after: old.attempts,
        link: Link::Opening,
        keepalive: Keepalive::Pinging,
    }))
}

/// `#[on_done(Connected)]`: completion of the parallel state, when both
/// regions are in a `#[final]` state. A parallel state has no single output,
/// so the handler gets only the fields (ADR 0005).
pub fn on_done_connected<S: Sink<()>>(
    up: ConnectedFields<'_>,
    cx: &mut ConnCx<'_, S>,
) -> Next<Conn, impl Edge<Conn> + use<S>> {
    note(cx, format!("on_done Connected (session {})", up.session));
    Next::Goto(ConnectedToClosed(|old: Connected| Receipt {
        session: old.session,
        frames: old.frames,
    }))
}
