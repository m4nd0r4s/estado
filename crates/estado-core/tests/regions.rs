//! Acceptance tests AC-02 (spec 5.6): exit and entry order of parallel
//! regions, the short-circuit, region-internal transitions applied before
//! the next region (observed through `cx.in_state`), consumption, defer and
//! bubbling to the parallel state's handler. On the hand-written `Conn`.

mod support;

use estado_core::IgnoreReason;
use support::connection::Event;
use support::connection::testing::{
    SESSION, connected, connected_data, handled_then_leave, lines, started,
};

#[test]
fn ac_02_entering_a_parallel_state_enters_the_regions_in_declaration_order() {
    let mut machine = started();
    machine.send(Event::Ring(SESSION)).unwrap();
    let log = lines(&mut machine);
    let start = log
        .iter()
        .position(|line| line == "entry Connected")
        .unwrap();
    assert_eq!(
        log[start..],
        ["entry Connected", "entry Opening", "entry Pinging"]
    );
}

#[test]
fn ac_02_leaving_from_a_region_leaf_exits_regions_in_reverse_order_then_the_parallel_state() {
    let mut machine = connected();
    machine.send(Event::Timeout).unwrap();
    machine.send(Event::Timeout).unwrap();
    assert_eq!(
        machine.path().to_string(),
        "Connected/link:Opening,keepalive:Dead"
    );
    lines(&mut machine);

    machine.send(Event::Reset).unwrap();

    // `link` does not handle `Reset`, so `keepalive` is evaluated and leaves.
    let mut expected = vec!["Opening handles Reset", "Dead handles Reset"];
    expected.extend(["exit Dead", "exit Opening", "exit Connected", "entry Idle"]);
    expected.push("entry Waiting");
    assert_eq!(lines(&mut machine), expected);
    assert_eq!(machine.path().to_string(), "Idle/Waiting");
}

#[test]
fn ac_02_a_transition_leaving_the_parallel_state_stops_the_evaluation_of_later_regions() {
    let mut machine = connected();
    machine.send(Event::LinkUp).unwrap();
    lines(&mut machine);

    machine.send(Event::Hangup).unwrap();

    // `Pinging` would leave too, but `link` came first: its handler is
    // never called.
    assert_eq!(
        lines(&mut machine),
        handled_then_leave(&["Open handles Hangup"], "Open", "Pinging")
    );
}

#[test]
fn ac_02_a_region_that_stays_does_not_stop_the_evaluation_of_later_regions() {
    let mut machine = connected();

    let report = machine.send(Event::Hangup).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        handled_then_leave(
            &["Opening handles Hangup", "Pinging handles Hangup"],
            "Opening",
            "Pinging"
        )
    );
    assert!(report.handled);
    assert_eq!(report.transitions, 1);
}

#[test]
fn ac_02_a_region_internal_transition_is_applied_before_the_next_region_is_evaluated() {
    let mut machine = connected();

    machine.send(Event::Sync).unwrap();

    // `keepalive` reads `link` through `cx.in_state`, and already sees `Open`
    // although the configuration was `Opening` when the microstep started.
    assert_eq!(
        lines(&mut machine),
        [
            "Opening handles Sync",
            "exit Opening",
            "entry Open",
            "Pinging handles Sync",
            "keepalive sees link Open",
            "exit Pinging",
            "entry AwaitingPong",
        ]
    );
    assert_eq!(
        machine.path().to_string(),
        "Connected/link:Open,keepalive:AwaitingPong"
    );
}

#[test]
fn ac_02_in_state_is_false_for_a_state_of_a_sibling_region_that_is_not_active() {
    let mut machine = connected();
    machine.send(Event::CloseLink).unwrap();
    lines(&mut machine);

    machine.send(Event::Sync).unwrap();

    // `Finished` has no handler, so only `keepalive` runs, and `Open` is not
    // active.
    assert_eq!(
        lines(&mut machine),
        ["Pinging handles Sync", "keepalive sees link not Open"]
    );
}

#[test]
fn ac_02_an_event_no_region_consumes_goes_to_the_parallel_state_handler() {
    let mut machine = connected();

    let report = machine.send(Event::Poke).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Opening handles Poke",
            "Pinging handles Poke",
            "Connected handles Poke"
        ]
    );
    assert_eq!(connected_data(&machine).frames, 1);
    assert!(report.handled);
    assert_eq!(report.transitions, 0);
}

#[test]
fn ac_02_the_parallel_state_handler_can_leave_and_exits_all_regions_in_reverse_order() {
    let mut machine = connected();

    machine.send(Event::Abort).unwrap();

    assert_eq!(
        lines(&mut machine),
        handled_then_leave(
            &[
                "Opening handles Abort",
                "Pinging handles Abort",
                "Connected handles Abort"
            ],
            "Opening",
            "Pinging"
        )
    );
}

#[test]
fn ac_02_an_event_nobody_handles_bubbles_out_of_the_parallel_state_and_is_ignored() {
    let mut machine = connected();

    let report = machine.send(Event::Ring(1)).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Opening handles Ring(1)",
            "Pinging handles Ring(1)",
            "Connected handles Ring(1)"
        ]
    );
    assert!(!report.handled);
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
}

#[test]
fn ac_02_the_event_is_deferred_when_every_region_defers_it() {
    let mut machine = connected();

    let report = machine.send(Event::Hold).unwrap().report;

    // The parallel state's own handler is not reached.
    assert_eq!(
        lines(&mut machine),
        ["Opening handles Hold", "Pinging handles Hold"]
    );
    assert!(report.handled);
    assert_eq!(machine.runtime().queues.deferred.len(), 1);
}

#[test]
fn ac_02_the_event_is_deferred_when_one_region_defers_and_the_other_does_not_handle_it() {
    let mut machine = connected();
    machine.send(Event::Timeout).unwrap();
    lines(&mut machine);

    machine.send(Event::Hold).unwrap();

    assert_eq!(
        lines(&mut machine),
        ["Opening handles Hold", "AwaitingPong handles Hold"]
    );
    assert_eq!(machine.runtime().queues.deferred.len(), 1);
}

#[test]
fn ac_02_a_region_that_consumes_the_event_wins_over_a_region_that_defers_it() {
    let mut machine = connected();
    machine.send(Event::LinkUp).unwrap();
    lines(&mut machine);

    machine.send(Event::Hold).unwrap();

    assert_eq!(
        lines(&mut machine),
        ["Open handles Hold", "Pinging handles Hold"]
    );
    assert_eq!(machine.runtime().queues.deferred.len(), 0);
}

#[test]
fn ac_02_a_deferred_event_is_redelivered_after_a_region_internal_transition() {
    let mut machine = connected();
    machine.send(Event::Hold).unwrap();
    lines(&mut machine);

    let report = machine.send(Event::LinkUp).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Opening handles LinkUp",
            "exit Opening",
            "entry Open",
            "Pinging handles LinkUp",
            "Open handles Hold",
            "Pinging handles Hold",
        ]
    );
    assert_eq!(report.microsteps, 2);
    assert_eq!(machine.runtime().queues.deferred.len(), 0);
}

#[test]
fn ac_02_reenter_in_a_region_counts_as_consumed_and_keeps_the_configuration() {
    let mut machine = connected();
    machine.send(Event::LinkUp).unwrap();
    lines(&mut machine);

    let report = machine.send(Event::Refresh).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Open handles Refresh",
            "exit Open",
            "entry Open",
            "Pinging handles Refresh"
        ]
    );
    assert!(report.handled);
    assert_eq!(report.transitions, 1);
    assert_eq!(
        machine.path().to_string(),
        "Connected/link:Open,keepalive:Pinging"
    );
}
