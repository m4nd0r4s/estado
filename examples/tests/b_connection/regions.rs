// The regions of `Connected`: keepalive, link, `cx.in_state` between them
// and the exit order when `Dead` leaves the parallel state (spec 5.3, 5.6).

use super::scenario::{Command, Conn, Event, PING_INTERVAL, PONG_TIMEOUT, backoff};
use estado_test::{DurationExt, assert_state};

use super::support::{
    SESSION, complete_dial, complete_send, connected, connected_data, frame, frames_in_flight,
    only_timer, path, take_log, transitions_only,
};

#[test]
fn a_pong_in_time_keeps_the_connection_alive() {
    let mut harness = connected();
    harness.advance(PING_INTERVAL);
    harness.advance(PONG_TIMEOUT - 1.millis());
    harness.send(Event::Pong);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
    // The next ping is due one interval after the pong, not before.
    let now = harness.now();
    assert_eq!(harness.pending_timers()[0].deadline, now + PING_INTERVAL);
}

#[test]
fn dead_leaves_all_regions_in_reverse_declaration_order() {
    let mut harness = connected();
    harness.advance(PING_INTERVAL);
    take_log(&mut harness);

    harness.advance(PONG_TIMEOUT);
    // `Dead` is entered, its entry raises `KeepaliveDead`, its handler leaves
    // `Connected`: `keepalive` exits first, then `link`, then `Connected`.
    assert_eq!(
        take_log(&mut harness),
        [
            "handler Idle",
            "handler AwaitingPong",
            "exit AwaitingPong",
            "entry Dead",
            "handler Idle",
            "handler Dead",
            "exit Dead",
            "exit Idle",
            "exit Connected",
            "emit Close { session: 7 }",
            "entry Backoff",
        ]
    );
    assert_state!(harness, Conn::Backoff { attempt: 0 });
}

#[test]
fn dead_cancels_the_timers_and_invokes_of_every_region() {
    let mut harness = connected();
    harness.send(Event::Send(frame("in flight")));
    assert_eq!(frames_in_flight(&harness).len(), 1);
    harness.advance(PING_INTERVAL);
    take_log(&mut harness);

    harness.advance(PONG_TIMEOUT);
    assert_eq!(
        transitions_only(take_log(&mut harness)),
        [
            "exit AwaitingPong",
            "entry Dead",
            "exit Dead",
            "exit Sending",
            "exit Connected",
            "emit Close { session: 7 }",
            "entry Backoff",
        ]
    );
    // The write in flight and the pong timer are gone; the backoff timer is
    // the only thing the new state scheduled.
    assert!(harness.pending_invokes().is_empty());
    let backoff_timer = only_timer(&harness);
    assert_eq!(backoff_timer.state, Conn::BACKOFF);
    assert_eq!(
        harness.pending_timers()[0].deadline,
        harness.now() + backoff(0)
    );
}

#[test]
fn after_the_loss_the_machine_reconnects_with_a_fresh_connected() {
    let mut harness = connected();
    harness.advance(PING_INTERVAL + PONG_TIMEOUT);
    assert_state!(harness, Conn::Backoff { attempt: 0 });
    harness.advance(backoff(0));
    assert_state!(harness, Conn::Connecting { attempt: 1 });
    complete_dial(&mut harness, SESSION);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:Pinging");
    assert_eq!(connected_data(&harness).frames_sent, 0, "a new instance");
}

#[test]
fn frame_sent_is_seen_by_both_regions_in_declaration_order() {
    let mut harness = connected();
    harness.advance(2.secs());
    harness.send(Event::Send(frame("data")));
    take_log(&mut harness);

    complete_send(&mut harness);
    // `link` goes `Sending -> Idle`, applied at once; then `keepalive`, which
    // also wants the event, re-enters `Pinging` (spec 5.6 items 1, 2 and 4).
    assert_eq!(
        take_log(&mut harness),
        [
            "handler Sending",
            "exit Sending",
            "entry Idle",
            "handler Pinging",
            "exit Pinging",
            "entry Pinging",
        ]
    );
    assert_eq!(
        harness.pending_timers()[0].deadline,
        2.secs() + PING_INTERVAL,
        "the ping interval started over"
    );
}

#[test]
fn the_link_refuses_to_send_while_keepalive_awaits_the_pong() {
    let mut harness = connected();
    harness.advance(PING_INTERVAL);
    take_log(&mut harness);

    let report = harness.send(Event::Send(frame("late"))).report;
    assert!(report.handled);
    assert_eq!(path(&harness), "Connected/link:Idle,keepalive:AwaitingPong");
    assert!(frames_in_flight(&harness).is_empty());
    assert_eq!(
        take_log(&mut harness),
        [
            "handler Idle",
            "link refuses to send while keepalive awaits the pong",
            "handler AwaitingPong",
        ]
    );

    // The pong changes the configuration: the deferred frame is redelivered
    // and now `in_state(AwaitingPong)` is false.
    harness.send(Event::Pong);
    assert_eq!(path(&harness), "Connected/link:Sending,keepalive:Pinging");
    assert_eq!(frames_in_flight(&harness), [frame("late")]);
}

#[test]
fn a_frame_deferred_by_the_link_survives_the_loss_of_the_connection() {
    let mut harness = connected();
    harness.advance(PING_INTERVAL);
    harness.send(Event::Send(frame("keep me")));

    // The pong never comes: `Connected` is torn down, the frame goes back to
    // the defer queue while `Backoff` waits...
    harness.advance(PONG_TIMEOUT);
    assert_state!(harness, Conn::Backoff { attempt: 0 });
    assert!(frames_in_flight(&harness).is_empty());

    // ...and is written once the next session is up.
    harness.advance(backoff(0));
    complete_dial(&mut harness, SESSION);
    assert_eq!(path(&harness), "Connected/link:Sending,keepalive:Pinging");
    assert_eq!(frames_in_flight(&harness), [frame("keep me")]);
}

#[test]
fn disconnect_emits_one_close_for_the_whole_parallel_state() {
    let mut harness = connected();
    let step = harness.send(Event::Disconnect);
    assert_eq!(step.effects, [Command::Close { session: 7 }]);
}
