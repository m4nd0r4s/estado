// Shared helpers of the scenario B tests.

use estado_core::{InvokeKey, Machine, TimerKey};
use super::scenario::{Command, Conn, Connected, Event, Frame, Session};
use estado_test::Harness;

pub const SESSION: Session = Session { id: 7 };

pub fn frame(text: &str) -> Frame {
    Frame::new(text)
}

/// `Disconnected`, the log cleared.
pub fn started() -> Harness<Conn> {
    let mut harness = Harness::new(Machine::<Conn>::new(Vec::new()));
    harness.start();
    take_log(&mut harness);
    harness
}

/// `Connecting { attempt: 0 }`, the log cleared.
pub fn connecting() -> Harness<Conn> {
    let mut harness = started();
    harness.send(Event::Connect);
    take_log(&mut harness);
    harness
}

/// `Connected/link:Idle,keepalive:Pinging` after the dial of session 7
/// succeeded, the log cleared.
pub fn connected() -> Harness<Conn> {
    let mut harness = connecting();
    complete_dial(&mut harness, SESSION);
    take_log(&mut harness);
    assert_eq!(
        harness.machine().path().to_string(),
        "Connected/link:Idle,keepalive:Pinging"
    );
    harness
}

/// Delivers the result of the pending dial.
pub fn complete_dial(harness: &mut Harness<Conn>, session: Session) {
    harness.complete_invoke(
        |task| matches!(task, Command::Dial { .. }),
        Event::Connected(session),
    );
}

/// Delivers the result of the pending frame write.
pub fn complete_send(harness: &mut Harness<Conn>) {
    harness.complete_invoke(
        |task| matches!(task, Command::SendFrame(_)),
        Event::FrameSent,
    );
}

/// Takes the log kept in the global context.
pub fn take_log(harness: &mut Harness<Conn>) -> Vec<String> {
    std::mem::take(harness.machine_mut().ctx_mut())
}

/// The log without the `handler ..` lines: entries, exits and effects.
pub fn transitions_only(log: Vec<String>) -> Vec<String> {
    log.into_iter()
        .filter(|line| !line.starts_with("handler"))
        .collect()
}

pub fn path(harness: &Harness<Conn>) -> String {
    harness.machine().path().to_string()
}

pub fn connected_data(harness: &Harness<Conn>) -> &Connected {
    match harness.machine().state() {
        Conn::Connected(connected) => connected,
        other => panic!("expected Connected, got {other:?}"),
    }
}

/// The frames of the pending `SendFrame` invokes, oldest first.
pub fn frames_in_flight(harness: &Harness<Conn>) -> Vec<Frame> {
    harness
        .pending_invokes()
        .iter()
        .filter_map(|invoke| match &invoke.task {
            Command::SendFrame(frame) => Some(frame.clone()),
            _ => None,
        })
        .collect()
}

/// The only pending timer.
pub fn only_timer(harness: &Harness<Conn>) -> TimerKey {
    match harness.pending_timers() {
        [timer] => timer.key,
        other => panic!("expected exactly one pending timer, got {other:?}"),
    }
}

/// The only pending invoke.
pub fn only_invoke(harness: &Harness<Conn>) -> InvokeKey {
    match harness.pending_invokes() {
        [invoke] => invoke.key,
        other => panic!("expected exactly one pending invoke, got {other:?}"),
    }
}
