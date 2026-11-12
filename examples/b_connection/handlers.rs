//! The user's handlers, as they look after `#[handlers]` rewrote each `goto!`
//! into a variant of the handler's edge enum (spec 6.2, 6.3, ADR 0001).
//!
//! Parameters come in the order of spec 6.2: own data, the `*Fields` of the
//! ancestors, `cx`, the event.

use estado_core::{Next, Sink};

use super::actions::{ConnCx, emit, note};
use super::edges::{
    AwaitingPongEdges, BackoffEdges, ConnectedEdges, ConnectingEdges, DeadEdges, DisconnectedEdges,
    IdleEdges, PingingEdges, SendingEdges,
};
use super::model::{Command, Conn, Event, MAX_ATTEMPTS};
use super::views::{BackoffMut, ConnectedFields, ConnectingMut, SendingMut};

/// `#[on(Disconnected)]`. `Send` is not handled here: a frame sent while
/// disconnected is ignored under the machine's `ignore` policy.
pub fn on_disconnected<S: Sink<Command>>(
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, DisconnectedEdges> {
    note(cx, "handler Disconnected");
    match event {
        Event::Connect => Next::Goto(DisconnectedEdges::ToConnecting),
        _ => Next::Bubble,
    }
}

/// `#[on(Connecting)]`: defers `Send` until there is a session.
pub fn on_connecting<S: Sink<Command>>(
    connecting: ConnectingMut<'_>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, ConnectingEdges> {
    note(cx, "handler Connecting");
    match event {
        Event::Connected(session) => Next::Goto(ConnectingEdges::ToConnected { session: *session }),
        Event::ConnectTimeout if *connecting.attempt + 1 >= MAX_ATTEMPTS => {
            emit(cx, Command::GiveUp);
            Next::Goto(ConnectingEdges::ToDisconnectedAfterGiveUp)
        }
        Event::ConnectTimeout => Next::Goto(ConnectingEdges::ToBackoff {
            attempt: *connecting.attempt,
        }),
        Event::Disconnect => Next::Goto(ConnectingEdges::ToDisconnected),
        Event::Send(_) => Next::Defer,
        _ => Next::Bubble,
    }
}

/// `#[on(Backoff)]`: defers `Send` too.
pub fn on_backoff<S: Sink<Command>>(
    backoff: BackoffMut<'_>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, BackoffEdges> {
    note(cx, "handler Backoff");
    match event {
        Event::Retry => Next::Goto(BackoffEdges::ToConnecting {
            attempt: *backoff.attempt + 1,
        }),
        Event::Disconnect => Next::Goto(BackoffEdges::ToDisconnected),
        Event::Send(_) => Next::Defer,
        _ => Next::Bubble,
    }
}

/// `#[on(Connected)]`: reached only when no region consumed or deferred the
/// event, so `Disconnect` lands here.
pub fn on_connected<S: Sink<Command>>(
    up: ConnectedFields<'_>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, ConnectedEdges> {
    note(
        cx,
        &format!("handler Connected (session {})", up.session.id),
    );
    match event {
        Event::Disconnect => Next::Goto(ConnectedEdges::ToDisconnected),
        _ => Next::Bubble,
    }
}

/// `#[on(Idle)]`, a leaf of `link`. Refuses to start a frame while the
/// `keepalive` region waits for a pong (it asks `cx.in_state`), by deferring
/// it: the pong changes the configuration, which redelivers the frame.
pub fn on_idle<S: Sink<Command>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Next<Conn, IdleEdges> {
    note(cx, "handler Idle");
    match event {
        Event::Send(_) if cx.in_state(Conn::AWAITING_PONG) => {
            note(cx, "link refuses to send while keepalive awaits the pong");
            Next::Defer
        }
        Event::Send(frame) => Next::Goto(IdleEdges::ToSending {
            frame: frame.clone(),
        }),
        _ => Next::Bubble,
    }
}

/// `#[on(Sending)]`: one frame in flight, the next `Send` waits.
pub fn on_sending<S: Sink<Command>>(
    _sending: SendingMut<'_>,
    up: ConnectedFields<'_>,
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, SendingEdges> {
    note(cx, "handler Sending");
    match event {
        Event::FrameSent => {
            *up.frames_sent += 1;
            Next::Goto(SendingEdges::ToIdle)
        }
        Event::Send(_) => Next::Defer,
        _ => Next::Bubble,
    }
}

/// `#[on(Pinging)]`, a leaf of `keepalive`. A delivered frame proves the
/// peer is alive, so `FrameSent` re-enters `Pinging`: a new epoch, and the
/// ping interval starts over.
pub fn on_pinging<S: Sink<Command>>(
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, PingingEdges> {
    note(cx, "handler Pinging");
    match event {
        Event::PingDue => Next::Goto(PingingEdges::ToAwaitingPong),
        Event::FrameSent => Next::Reenter,
        _ => Next::Bubble,
    }
}

/// `#[on(AwaitingPong)]`.
pub fn on_awaiting_pong<S: Sink<Command>>(
    cx: &mut ConnCx<'_, S>,
    event: &Event,
) -> Next<Conn, AwaitingPongEdges> {
    note(cx, "handler AwaitingPong");
    match event {
        Event::Pong => Next::Goto(AwaitingPongEdges::ToPinging),
        Event::PongTimeout => Next::Goto(AwaitingPongEdges::ToDead),
        _ => Next::Bubble,
    }
}

/// `#[on(Dead)]`: leaves `Connected`, so every region exits.
pub fn on_dead<S: Sink<Command>>(cx: &mut ConnCx<'_, S>, event: &Event) -> Next<Conn, DeadEdges> {
    note(cx, "handler Dead");
    match event {
        Event::KeepaliveDead => Next::Goto(DeadEdges::ToBackoff),
        _ => Next::Bubble,
    }
}
