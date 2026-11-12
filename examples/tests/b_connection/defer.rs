//! Deferral of `Send` and the defer queue (spec 5.4, 5.5).

use estado_core::{DeferOverflowPolicy, IgnoreReason, SendError, Spec};
use estado_examples::b_connection::{
    CONNECT_TIMEOUT, Conn, DEFER_CAP, DEFER_OVERFLOW_POLICY, Event, Frame,
};
use estado_test::{DurationExt, assert_state};

use super::support::{
    SESSION, complete_dial, complete_send, connected, connected_data, connecting, frame,
    frames_in_flight, path, started,
};

/// Completes the frame writes one after the other and returns the frames in
/// the order they were written.
fn deliver_in_order(harness: &mut estado_test::Harness<Conn>) -> Vec<Frame> {
    let mut written = Vec::new();
    while let Some(next) = frames_in_flight(harness).first().cloned() {
        written.push(next);
        complete_send(harness);
    }
    written
}

#[test]
fn send_is_deferred_while_connecting_and_redelivered_in_order_after_connect() {
    let mut harness = connecting();
    for text in ["one", "two", "three"] {
        let step = harness.send(Event::Send(frame(text)));
        assert!(step.report.handled);
        step.expect_no_effect();
    }
    assert_state!(harness, Conn::Connecting { .. });
    assert!(frames_in_flight(&harness).is_empty());

    complete_dial(&mut harness, SESSION);
    // Only one frame is in flight at a time; the others wait in the queue.
    assert_eq!(frames_in_flight(&harness), [frame("one")]);
    assert_eq!(
        deliver_in_order(&mut harness),
        [frame("one"), frame("two"), frame("three")]
    );
    assert_eq!(connected_data(&harness).frames_sent, 3);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
}

#[test]
fn send_is_deferred_while_backing_off_and_survives_the_retries() {
    let mut harness = connecting();
    harness.send(Event::Send(frame("one")));
    harness.advance(CONNECT_TIMEOUT);
    assert_state!(harness, Conn::Backoff { attempt: 0 });
    harness.send(Event::Send(frame("two")));

    harness.advance(100.millis());
    assert_state!(harness, Conn::Connecting { attempt: 1 });
    complete_dial(&mut harness, SESSION);
    assert_eq!(deliver_in_order(&mut harness), [frame("one"), frame("two")]);
}

#[test]
fn a_busy_link_defers_the_next_send_too() {
    let mut harness = connected();
    harness.send(Event::Send(frame("one")));
    let report = harness.send(Event::Send(frame("two"))).report;
    assert!(report.handled);
    assert_eq!(frames_in_flight(&harness), [frame("one")]);
    assert_eq!(deliver_in_order(&mut harness), [frame("one"), frame("two")]);
}

#[test]
fn defer_overflow_follows_the_declared_policy() {
    // The policy is spelled out as a constant of the machine: `error`.
    assert_eq!(DEFER_OVERFLOW_POLICY, DeferOverflowPolicy::Error);
    assert_eq!(<Conn as Spec>::DEFER_OVERFLOW, DEFER_OVERFLOW_POLICY);
    assert_eq!(DEFER_CAP, 4);

    let mut harness = connecting();
    let texts = ["f1", "f2", "f3", "f4"];
    for text in texts {
        harness.send(Event::Send(frame(text)));
    }

    // The fifth does not fit: it comes back to the caller, nothing changed.
    let refused = harness
        .machine_mut()
        .send(Event::Send(frame("f5")))
        .unwrap_err();
    assert_eq!(refused, SendError::DeferOverflow(Event::Send(frame("f5"))));
    assert!(!harness.machine().is_poisoned());
    assert_state!(harness, Conn::Connecting { attempt: 0 });

    // The four that fit are delivered, in order; the fifth is not.
    complete_dial(&mut harness, SESSION);
    assert_eq!(
        deliver_in_order(&mut harness),
        texts.map(frame),
        "no frame was lost and none was invented"
    );
}

#[test]
fn a_refused_send_can_be_resent_once_there_is_room() {
    let mut harness = connecting();
    for text in ["f1", "f2", "f3", "f4"] {
        harness.send(Event::Send(frame(text)));
    }
    assert!(
        harness
            .machine_mut()
            .send(Event::Send(frame("f5")))
            .is_err()
    );

    complete_dial(&mut harness, SESSION);
    // One frame is in flight, three wait: there is room for one more.
    harness.send(Event::Send(frame("f5")));
    assert_eq!(
        deliver_in_order(&mut harness),
        ["f1", "f2", "f3", "f4", "f5"].map(frame)
    );
}

#[test]
fn disconnect_discards_the_deferred_frames() {
    let mut harness = connecting();
    harness.send(Event::Send(frame("one")));
    harness.send(Event::Send(frame("two")));

    // `Disconnected` changes the configuration, so both frames are
    // redelivered to a state that does not handle `Send`: counted, dropped.
    let report = harness.send(Event::Disconnect).report;
    assert_eq!(report.unhandled_internal, 2);
    assert_state!(harness, Conn::Disconnected);

    harness.send(Event::Connect);
    complete_dial(&mut harness, SESSION);
    assert!(
        frames_in_flight(&harness).is_empty(),
        "nothing left to send"
    );
}

#[test]
fn send_while_disconnected_is_not_deferred_but_ignored() {
    let mut harness = started();
    let report = harness.send(Event::Send(frame("nobody listens"))).report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    harness.send(Event::Connect);
    complete_dial(&mut harness, SESSION);
    assert!(frames_in_flight(&harness).is_empty());
}

#[test]
fn deferring_costs_one_microstep_per_event_and_configuration_change() {
    let mut harness = connecting();
    for text in ["one", "two"] {
        harness.send(Event::Send(frame(text)));
    }
    // The connect timeout changes the configuration: both frames are
    // redelivered to `Backoff`, which defers them again.
    let report = harness.advance(CONNECT_TIMEOUT).report;
    assert_eq!(report.transitions, 1);
    assert_eq!(report.microsteps, 1 + 2);
}
