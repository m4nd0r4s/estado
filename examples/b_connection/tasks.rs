//! Timers and invokes of the states (spec 5.3 steps 2, 8 and 9, spec 7.1,
//! 7.2): one pair of functions per state with `#[after]` or `#[invoke]`.
//!
//! A key is the state, its epoch and the index of the timer or invoke inside
//! the state. `start_*` moves the epoch on and schedules; `cancel_*` uses the
//! epoch of the instance that is leaving. The generated code bumps every
//! epoch of the transition before it emits (step 8, then 9); doing both per
//! state is equivalent because the epochs are independent counters.

use estado_core::{ChildPath, Effect, InvokeKey, Sink, StateId, TimerKey};

use super::actions::ConnCx;
use super::model::{
    CONNECT_TIMEOUT, Command, Conn, Connected, Frame, Keepalive, Link, PING_INTERVAL, PONG_TIMEOUT,
    backoff,
};
use super::runtime::next_epoch;

const fn timer(state: StateId, epoch: u32) -> TimerKey {
    TimerKey {
        state,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

const fn invoke(state: StateId, epoch: u32) -> InvokeKey {
    InvokeKey {
        state,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

/// `Connecting`'s `#[after]`.
pub(super) const fn connect_timer(epoch: u32) -> TimerKey {
    timer(Conn::CONNECTING, epoch)
}

/// `Connecting`'s `#[invoke]` (the dial).
pub(super) const fn dial_invoke(epoch: u32) -> InvokeKey {
    invoke(Conn::CONNECTING, epoch)
}

/// `Backoff`'s `#[after]`.
pub(super) const fn backoff_timer(epoch: u32) -> TimerKey {
    timer(Conn::BACKOFF, epoch)
}

/// `Sending`'s `#[invoke]`.
pub(super) const fn send_invoke(epoch: u32) -> InvokeKey {
    invoke(Conn::SENDING, epoch)
}

/// `Pinging`'s `#[after]`.
pub(super) const fn ping_timer(epoch: u32) -> TimerKey {
    timer(Conn::PINGING, epoch)
}

/// `AwaitingPong`'s `#[after]`.
pub(super) const fn pong_timer(epoch: u32) -> TimerKey {
    timer(Conn::AWAITING_PONG, epoch)
}

fn start_connecting<S: Sink<Command>>(attempt: u8, cx: &mut ConnCx<'_, S>) {
    let epoch = next_epoch(&mut cx.runtime_mut().connecting_epoch);
    cx.effect(Effect::StartTimer {
        key: connect_timer(epoch),
        after: CONNECT_TIMEOUT,
    });
    cx.effect(Effect::Invoke {
        key: dial_invoke(epoch),
        task: Command::Dial { attempt },
    });
}

fn cancel_connecting<S: Sink<Command>>(cx: &mut ConnCx<'_, S>) {
    let epoch = cx.runtime_mut().connecting_epoch;
    cx.effect(Effect::CancelTimer(connect_timer(epoch)));
    cx.effect(Effect::Cancel(dial_invoke(epoch)));
}

fn start_backoff<S: Sink<Command>>(attempt: u8, cx: &mut ConnCx<'_, S>) {
    let epoch = next_epoch(&mut cx.runtime_mut().backoff_epoch);
    cx.effect(Effect::StartTimer {
        key: backoff_timer(epoch),
        after: backoff(attempt),
    });
}

fn cancel_backoff<S: Sink<Command>>(cx: &mut ConnCx<'_, S>) {
    let epoch = cx.runtime_mut().backoff_epoch;
    cx.effect(Effect::CancelTimer(backoff_timer(epoch)));
}

fn start_sending<S: Sink<Command>>(frame: &Frame, cx: &mut ConnCx<'_, S>) {
    let epoch = next_epoch(&mut cx.runtime_mut().sending_epoch);
    cx.effect(Effect::Invoke {
        key: send_invoke(epoch),
        task: Command::SendFrame(frame.clone()),
    });
}

fn cancel_sending<S: Sink<Command>>(cx: &mut ConnCx<'_, S>) {
    let epoch = cx.runtime_mut().sending_epoch;
    cx.effect(Effect::Cancel(send_invoke(epoch)));
}

/// Tasks of the entered leaf of the `link` region.
pub(super) fn start_link<S: Sink<Command>>(link: &Link, cx: &mut ConnCx<'_, S>) {
    match link {
        Link::Idle => {}
        Link::Sending { frame } => start_sending(frame, cx),
    }
}

/// Tasks of the exiting leaf of the `link` region.
pub(super) fn cancel_link<S: Sink<Command>>(link: &Link, cx: &mut ConnCx<'_, S>) {
    match link {
        Link::Idle => {}
        Link::Sending { .. } => cancel_sending(cx),
    }
}

/// Tasks of the entered leaf of the `keepalive` region.
pub(super) fn start_keepalive<S: Sink<Command>>(keepalive: Keepalive, cx: &mut ConnCx<'_, S>) {
    match keepalive {
        Keepalive::Pinging => {
            let epoch = next_epoch(&mut cx.runtime_mut().pinging_epoch);
            cx.effect(Effect::StartTimer {
                key: ping_timer(epoch),
                after: PING_INTERVAL,
            });
        }
        Keepalive::AwaitingPong => {
            let epoch = next_epoch(&mut cx.runtime_mut().awaiting_pong_epoch);
            cx.effect(Effect::StartTimer {
                key: pong_timer(epoch),
                after: PONG_TIMEOUT,
            });
        }
        Keepalive::Dead => {}
    }
}

/// Tasks of the exiting leaf of the `keepalive` region.
pub(super) fn cancel_keepalive<S: Sink<Command>>(keepalive: Keepalive, cx: &mut ConnCx<'_, S>) {
    match keepalive {
        Keepalive::Pinging => {
            let epoch = cx.runtime_mut().pinging_epoch;
            cx.effect(Effect::CancelTimer(ping_timer(epoch)));
        }
        Keepalive::AwaitingPong => {
            let epoch = cx.runtime_mut().awaiting_pong_epoch;
            cx.effect(Effect::CancelTimer(pong_timer(epoch)));
        }
        Keepalive::Dead => {}
    }
}

/// Schedules the tasks of everything that just entered, parents first and
/// regions in declaration order (spec 5.3 step 9).
pub(super) fn start_root<S: Sink<Command>>(root: &Conn, cx: &mut ConnCx<'_, S>) {
    match root {
        Conn::Disconnected => {}
        Conn::Connecting { attempt } => start_connecting(*attempt, cx),
        Conn::Backoff { attempt } => start_backoff(*attempt, cx),
        Conn::Connected(Connected {
            link, keepalive, ..
        }) => {
            start_link(link, cx);
            start_keepalive(*keepalive, cx);
        }
    }
}

/// Cancels the tasks of everything that just exited, deepest first and
/// regions in reverse declaration order (spec 5.3 step 2).
pub(super) fn cancel_root<S: Sink<Command>>(root: &Conn, cx: &mut ConnCx<'_, S>) {
    match root {
        Conn::Disconnected => {}
        Conn::Connecting { .. } => cancel_connecting(cx),
        Conn::Backoff { .. } => cancel_backoff(cx),
        Conn::Connected(Connected {
            link, keepalive, ..
        }) => {
            cancel_keepalive(*keepalive, cx);
            cancel_link(link, cx);
        }
    }
}
