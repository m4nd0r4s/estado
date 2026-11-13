// The test cases of scenario B, shared by the hand-written version and the
// `#[machine]` version: the including module provides `scenario` (the module
// under test).
mod defer {
    include!("defer.rs");
}
mod regions {
    include!("regions.rs");
}
mod retry {
    include!("retry.rs");
}
mod stale {
    include!("stale.rs");
}
#[allow(dead_code)]
mod support {
    include!("support.rs");
}

use estado_core::IgnoreReason;
use scenario::{CONNECT_TIMEOUT, Command, Conn, Event, PING_INTERVAL, PONG_TIMEOUT};
use estado_test::{DurationExt, assert_state};

use support::{
    SESSION, complete_dial, complete_send, connected, connected_data, connecting, frame,
    only_invoke, only_timer, path, started, take_log, transitions_only,
};

#[test]
fn start_enters_disconnected() {
    let mut harness = estado_test::Harness::new(estado_core::Machine::<Conn>::new(Vec::new()));
    harness.start().expect_no_effect();
    assert_eq!(path(&harness), "Disconnected");
    assert_eq!(*harness.machine().ctx(), ["entry Disconnected"]);
    harness.assert_no_pending_timers();
}

#[test]
fn connect_dials_and_arms_the_connect_timeout() {
    let mut harness = started();
    harness.send(Event::Connect).expect_no_effect();
    assert_state!(harness, Conn::Connecting { attempt: 0 });
    assert_eq!(
        take_log(&mut harness),
        [
            "handler Disconnected",
            "exit Disconnected",
            "entry Connecting"
        ]
    );

    // Both tasks belong to the same instance of `Connecting`.
    let timer = harness.pending_timers()[0].clone();
    assert_eq!(timer.deadline, CONNECT_TIMEOUT);
    assert_eq!(timer.key.epoch, 1);
    assert_eq!(harness.pending_invokes().len(), 1);
    assert_eq!(
        harness.pending_invokes()[0].task,
        Command::Dial { attempt: 0 }
    );
    assert_eq!(harness.pending_invokes()[0].key.epoch, 1);
}

#[test]
fn a_successful_dial_enters_connected_with_both_regions() {
    let mut harness = connecting();
    complete_dial(&mut harness, SESSION);

    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
    assert_eq!(connected_data(&harness).session, SESSION);
    assert_eq!(
        take_log(&mut harness),
        [
            "handler Connecting",
            "exit Connecting",
            "entry Connected",
            "entry Idle",
            "entry Pinging",
        ]
    );
    // The dial and its timer were cancelled by the exit; only the ping
    // interval of the new keepalive region is pending.
    assert!(harness.pending_invokes().is_empty());
    assert_eq!(only_timer(&harness).state, Conn::PINGING_ID);
}

#[test]
fn happy_path_sends_a_frame_pings_and_disconnects() {
    let mut harness = connected();

    // A frame goes out through the `link` invoke.
    harness.send(Event::Send(frame("hello"))).expect_no_effect();
    assert_eq!(path(&harness), "Connected/link:Sending,keepalive:Pinging");
    assert_eq!(
        harness.pending_invokes()[0].task,
        Command::SendFrame(frame("hello"))
    );
    complete_send(&mut harness);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
    assert_eq!(connected_data(&harness).frames_sent, 1);

    // The ping interval elapsed since the frame was delivered.
    harness
        .advance(PING_INTERVAL)
        .expect_effect(|command| *command == Command::Ping);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:AwaitingPong");
    harness.send(Event::Pong).expect_no_effect();
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");

    // Hanging up closes the session and leaves nothing scheduled.
    take_log(&mut harness);
    harness
        .send(Event::Disconnect)
        .expect_effect(|command| *command == Command::Close { session: 7 });
    assert_state!(harness, Conn::Disconnected);
    harness.assert_no_pending_timers();
    assert!(harness.pending_invokes().is_empty());
}

#[test]
fn a_silent_peer_is_declared_dead_after_ping_and_pong_timeout() {
    let mut harness = connected();
    harness.advance(PING_INTERVAL);
    harness.advance(PONG_TIMEOUT - 1.millis());
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:AwaitingPong");
    harness
        .advance(1.millis())
        .expect_effect(|command| *command == Command::Close { session: 7 });
    assert_state!(harness, Conn::Backoff { attempt: 0 });
}

#[test]
fn disconnect_is_handled_by_the_parallel_state_itself() {
    let mut harness = connected();
    harness.send(Event::Disconnect);
    assert_eq!(
        take_log(&mut harness),
        [
            "handler Idle",
            "handler Pinging",
            "handler Connected (session 7)",
            "exit Pinging",
            "exit Idle",
            "exit Connected",
            "emit Close { session: 7 }",
            "entry Disconnected",
        ]
    );
}

#[test]
fn events_nobody_handles_are_ignored() {
    let mut harness = started();
    let report = harness.send(Event::Send(frame("early"))).report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert!(!report.handled);
    assert_state!(harness, Conn::Disconnected);

    let mut harness = connected();
    let report = harness.send(Event::Connect).report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert_eq!(
        transitions_only(take_log(&mut harness)),
        Vec::<String>::new()
    );
    assert!(harness.pending_invokes().is_empty());
}

#[test]
fn the_connect_timer_and_the_dial_share_an_epoch() {
    let harness = connecting();
    assert_eq!(only_timer(&harness).epoch, only_invoke(&harness).epoch);
}
