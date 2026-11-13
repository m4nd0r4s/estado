// Epochs: late timeouts and late invoke results are `Stale` (spec 7.1, 7.2).

use estado_core::IgnoreReason;
use super::scenario::{CONNECT_TIMEOUT, Command, Conn, Event, PING_INTERVAL};
use estado_test::{DurationExt, assert_state};

use super::support::{
    SESSION, complete_dial, complete_send, connected, connecting, frame, only_invoke, only_timer,
    path, take_log,
};

#[test]
fn a_connect_timeout_that_arrives_after_connected_is_stale() {
    let mut harness = connecting();
    // The key of the connect timer, copied before the exit cancels it.
    let late_timeout = only_timer(&harness);
    complete_dial(&mut harness, SESSION);
    assert!(
        harness
            .pending_timers()
            .iter()
            .all(|timer| timer.key != late_timeout),
        "the exit cancelled it"
    );
    take_log(&mut harness);

    // The runtime lost the race: the timer fires anyway.
    let step = harness.machine_mut().fire(late_timeout).unwrap();
    assert_eq!(step.report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(step.report.microsteps, 0, "no microstep ran");
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
    assert!(take_log(&mut harness).is_empty(), "no handler ran");
}

#[test]
fn a_timer_of_an_earlier_instance_of_the_same_state_is_stale() {
    let mut harness = connecting();
    let first_instance = only_timer(&harness);
    assert_eq!(first_instance.epoch, 1);

    // Connecting -> Backoff -> Connecting: the state is the same, the epoch
    // is not.
    harness.advance(CONNECT_TIMEOUT + 100.millis());
    assert_state!(harness, Conn::Connecting { attempt: 1 });
    let second_instance = only_timer(&harness);
    assert_eq!(second_instance.state, first_instance.state);
    assert_eq!(second_instance.epoch, 2);

    let step = harness.machine_mut().fire(first_instance).unwrap();
    assert_eq!(step.report.ignored, Some(IgnoreReason::Stale));
    assert_state!(harness, Conn::Connecting { attempt: 1 });

    // The current instance still works.
    let step = harness.machine_mut().fire(second_instance).unwrap();
    assert_eq!(step.report.ignored, None);
    assert_state!(harness, Conn::Backoff { attempt: 1 });
}

#[test]
fn a_late_dial_result_of_an_earlier_attempt_is_stale() {
    let mut harness = connecting();
    let first_dial = only_invoke(&harness);
    harness.advance(CONNECT_TIMEOUT + 100.millis());
    assert_state!(harness, Conn::Connecting { attempt: 1 });

    // The first dial finally answers, while the second one is running.
    let report = harness
        .complete_key(first_dial, Event::Connected(SESSION))
        .report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_state!(harness, Conn::Connecting { attempt: 1 });

    // The answer of the second dial is accepted.
    complete_dial(&mut harness, SESSION);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
}

#[test]
fn a_late_dial_result_after_giving_up_is_stale() {
    let mut harness = connecting();
    let dial = only_invoke(&harness);
    harness.send(Event::Disconnect);
    assert_state!(harness, Conn::Disconnected);
    let report = harness.complete_key(dial, Event::Connected(SESSION)).report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_state!(harness, Conn::Disconnected);
}

#[test]
fn a_late_frame_result_after_the_link_left_sending_is_stale() {
    let mut harness = connected();
    harness.send(Event::Send(frame("one")));
    let first_write = only_invoke(&harness);
    complete_send(&mut harness);
    harness.send(Event::Send(frame("two")));
    assert_eq!(
        harness.pending_invokes()[0].task,
        Command::SendFrame(frame("two"))
    );

    let report = harness.complete_key(first_write, Event::FrameSent).report;
    assert_eq!(report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(path(&harness), "Connected/link:Sending,keepalive:Pinging");
}

#[test]
fn a_late_pong_timeout_after_the_pong_is_stale() {
    let mut harness = connected();
    harness.advance(PING_INTERVAL);
    let pong_deadline = only_timer(&harness);
    assert_eq!(pong_deadline.state, Conn::AWAITING_PONG_ID);
    harness.send(Event::Pong);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");

    let step = harness.machine_mut().fire(pong_deadline).unwrap();
    assert_eq!(step.report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
}

#[test]
fn reentering_pinging_gives_the_ping_timer_a_new_epoch() {
    let mut harness = connected();
    let first_ping_timer = only_timer(&harness);
    harness.advance(3.secs());

    // A delivered frame is activity: `Pinging` is re-entered.
    harness.send(Event::Send(frame("data")));
    complete_send(&mut harness);
    let second_ping_timer = only_timer(&harness);
    assert_eq!(second_ping_timer.state, first_ping_timer.state);
    assert_eq!(second_ping_timer.epoch, first_ping_timer.epoch + 1);

    let step = harness.machine_mut().fire(first_ping_timer).unwrap();
    assert_eq!(step.report.ignored, Some(IgnoreReason::Stale));
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
}
