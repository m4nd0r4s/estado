//! Retries with exponential backoff, driven by the virtual clock.

use estado_examples::b_connection::{
    BACKOFF_CAP, CONNECT_TIMEOUT, Command, Conn, Event, MAX_ATTEMPTS, backoff,
};
use estado_test::{DurationExt, assert_state};

use super::support::{
    SESSION, complete_dial, connecting, frames_in_flight, only_timer, path, take_log,
};

#[test]
fn backoff_doubles_with_every_attempt_and_is_capped() {
    let millis = |attempt| backoff(attempt).as_millis();
    assert_eq!(
        [0, 1, 2, 3, 4].map(millis),
        [100, 200, 400, 800, 1000],
        "100 ms * 2^attempt, the fifth step already hits the 1 s cap"
    );
    assert_eq!(backoff(5), BACKOFF_CAP);
    assert_eq!(
        backoff(u8::MAX),
        BACKOFF_CAP,
        "no overflow for large attempts"
    );
}

#[test]
fn a_connect_timeout_backs_off_and_the_retry_dials_again() {
    let mut harness = connecting();

    harness.advance(CONNECT_TIMEOUT - 1.millis());
    assert_state!(harness, Conn::Connecting { attempt: 0 });
    harness.advance(1.millis());
    assert_state!(harness, Conn::Backoff { attempt: 0 });
    assert_eq!(harness.now(), CONNECT_TIMEOUT);
    // `Connecting` left: its timer and its dial are cancelled; the backoff
    // timer is the only pending one.
    assert!(harness.pending_invokes().is_empty());
    assert_eq!(only_timer(&harness).state, Conn::BACKOFF);
    assert_eq!(
        harness.pending_timers()[0].deadline,
        CONNECT_TIMEOUT + 100.millis()
    );

    harness.advance(99.millis());
    assert_state!(harness, Conn::Backoff { attempt: 0 });
    harness.advance(1.millis());
    assert_state!(harness, Conn::Connecting { attempt: 1 });
    assert_eq!(
        harness.pending_invokes()[0].task,
        Command::Dial { attempt: 1 }
    );
}

#[test]
fn every_retry_waits_twice_as_long_until_the_cap() {
    let mut harness = connecting();
    let mut waits = Vec::new();
    for attempt in 0..MAX_ATTEMPTS - 1 {
        harness.advance(CONNECT_TIMEOUT);
        assert_state!(harness, Conn::Backoff { attempt: failed } if *failed == attempt);
        let waiting_since = harness.now();
        let deadline = harness.pending_timers()[0].deadline;
        waits.push((deadline - waiting_since).as_millis());
        harness.advance(deadline - waiting_since);
        assert_state!(harness, Conn::Connecting { attempt: next } if *next == attempt + 1);
    }
    assert_eq!(waits, [100, 200, 400, 800, 1000]);
}

#[test]
fn the_last_failed_attempt_gives_up() {
    let mut harness = connecting();
    for _ in 0..MAX_ATTEMPTS - 1 {
        harness.advance(CONNECT_TIMEOUT);
        let wait = harness.pending_timers()[0].deadline - harness.now();
        harness.advance(wait);
    }
    assert_state!(harness, Conn::Connecting { attempt } if *attempt == MAX_ATTEMPTS - 1);

    take_log(&mut harness);
    harness
        .advance(CONNECT_TIMEOUT)
        .expect_effect(|command| *command == Command::GiveUp);
    assert_state!(harness, Conn::Disconnected);
    harness.assert_no_pending_timers();
    assert!(harness.pending_invokes().is_empty());
}

#[test]
fn a_dial_that_succeeds_after_retries_connects() {
    let mut harness = connecting();
    harness.advance(CONNECT_TIMEOUT + 100.millis());
    assert_state!(harness, Conn::Connecting { attempt: 1 });
    complete_dial(&mut harness, SESSION);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
    assert!(frames_in_flight(&harness).is_empty());
}

#[test]
fn disconnect_while_connecting_or_backing_off_cancels_everything() {
    let mut connecting_harness = connecting();
    connecting_harness.send(Event::Disconnect);
    assert_state!(connecting_harness, Conn::Disconnected);
    connecting_harness.assert_no_pending_timers();
    assert!(connecting_harness.pending_invokes().is_empty());

    let mut backing_off = connecting();
    backing_off.advance(CONNECT_TIMEOUT);
    assert_state!(backing_off, Conn::Backoff { .. });
    backing_off.send(Event::Disconnect);
    assert_state!(backing_off, Conn::Disconnected);
    backing_off.assert_no_pending_timers();

    // And a fresh cycle starts from attempt 0 again.
    backing_off.send(Event::Connect);
    assert_state!(backing_off, Conn::Connecting { attempt: 0 });
}
