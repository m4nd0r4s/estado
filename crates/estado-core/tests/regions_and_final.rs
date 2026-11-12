//! Acceptance tests AC-10 (spec 5.10, 5.6 item 6): `on_done` of compound
//! states, completion of a parallel state, root final and `is_done()`; plus
//! the active path and graph of regions. On the hand-written `Conn`.

mod support;

use estado_core::{IgnoreReason, StateKind};
use support::connection::testing::{SESSION, connected, lines, started};
use support::connection::{Conn, Connected, Event, Idle, IdleSub, Keepalive, Link, Receipt};

#[test]
fn ac_02_the_path_lists_the_active_configuration_in_pre_order() {
    let mut machine = connected();
    machine.send(Event::LinkUp).unwrap();

    let path = machine.path();
    assert_eq!(
        path.states(),
        [
            Conn::CONNECTED,
            Conn::LINK,
            Conn::OPEN,
            Conn::KEEPALIVE,
            Conn::PINGING
        ]
    );
    assert_eq!(path.to_string(), "Connected/link:Open,keepalive:Pinging");
    assert_eq!(
        estado_core::Spec::state_id(machine.state()),
        Conn::CONNECTED
    );
    assert!(machine.is_in(Conn::KEEPALIVE));
    assert!(machine.is_in(Conn::OPEN));
    assert!(!machine.is_in(Conn::OPENING));
    assert!(!machine.is_in(Conn::READY));
}

#[test]
fn ac_02_regions_are_real_states_of_the_graph() {
    let graph = <Conn as estado_core::Spec>::GRAPH;
    for region in [Conn::LINK, Conn::KEEPALIVE] {
        let info = graph.state(region);
        assert_eq!(info.kind, StateKind::Region);
        assert_eq!(info.parent, Some(Conn::CONNECTED));
    }
    assert_eq!(graph.state(Conn::CONNECTED).kind, StateKind::Parallel);
    assert_eq!(graph.state(Conn::OPEN).parent, Some(Conn::LINK));
    assert_eq!(graph.state(Conn::LINK).initial, Some(Conn::OPENING));
    assert_eq!(graph.state(Conn::KEEPALIVE).initial, Some(Conn::PINGING));
}

#[test]
fn ac_10_on_done_of_a_compound_runs_when_its_substate_enters_a_final() {
    let mut machine = started();
    machine.send(Event::Retry).unwrap();
    lines(&mut machine);

    let report = machine.send(Event::Ring(SESSION)).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Waiting handles Ring(7)",
            "exit Waiting",
            "entry Ready",
            "on_done Idle (output 7, attempts 1)",
            "exit Ready",
            "exit Idle",
            "entry Connected",
            "entry Opening",
            "entry Pinging",
        ]
    );
    // The constructor received `Idle` by value and carried its data over.
    assert_eq!(
        *machine.state(),
        Conn::Connected(Connected {
            session: SESSION,
            frames: 0,
            resumed_after: 1,
            link: Link::Opening,
            keepalive: Keepalive::Pinging,
        })
    );
    assert_eq!(report.microsteps, 2);
    assert_eq!(report.transitions, 2);
}

#[test]
fn ac_10_a_compound_without_on_done_stays_in_its_final_substate() {
    let mut machine = connected();
    machine.send(Event::Drain).unwrap();
    assert_eq!(machine.path().to_string(), "Draining/Flushing");
    lines(&mut machine);

    let report = machine.send(Event::Flush).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        ["Flushing handles Flush", "exit Flushing", "entry Flushed"]
    );
    assert_eq!(machine.path().to_string(), "Draining/Flushed");
    // The completion check ran as a microstep and found nothing to do.
    assert_eq!(report.microsteps, 2);
    assert_eq!(report.transitions, 1);
    assert!(!machine.is_done());

    // The final substate has no handler; the compound's own handler leaves.
    machine.send(Event::Reset).unwrap();
    assert_eq!(
        lines(&mut machine),
        [
            "Draining handles Reset",
            "exit Flushed",
            "exit Draining",
            "entry Idle",
            "entry Waiting"
        ]
    );
}

#[test]
fn ac_10_a_parallel_state_is_not_complete_while_one_region_is_still_running() {
    let mut machine = connected();

    let report = machine.send(Event::CloseLink).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Opening handles CloseLink",
            "exit Opening",
            "entry Finished",
            "Pinging handles CloseLink"
        ]
    );
    assert_eq!(
        machine.path().to_string(),
        "Connected/link:Finished,keepalive:Pinging"
    );
    assert_eq!(report.microsteps, 2);
    assert!(!machine.is_done());
}

#[test]
fn ac_10_a_parallel_state_completes_when_the_last_region_enters_a_final() {
    let mut machine = connected();
    machine.send(Event::CloseLink).unwrap();
    lines(&mut machine);

    machine.send(Event::Close).unwrap();

    // The finished `link` region has no handler: only `keepalive` runs.
    assert_eq!(
        lines(&mut machine),
        [
            "Pinging handles Close",
            "exit Pinging",
            "entry Stopped",
            "on_done Connected (session 7)",
            "exit Stopped",
            "exit Finished",
            "exit Connected",
            "entry Closed",
        ]
    );
    assert!(machine.is_done());
}

#[test]
fn ac_10_regions_finishing_in_one_event_complete_the_parallel_state_after_both_entries() {
    let mut machine = connected();
    machine.send(Event::LinkUp).unwrap();
    machine.send(Event::Poke).unwrap();
    lines(&mut machine);

    let report = machine.send(Event::Close).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Open handles Close",
            "exit Open",
            "entry Finished",
            "Pinging handles Close",
            "exit Pinging",
            "entry Stopped",
            "on_done Connected (session 7)",
            "exit Stopped",
            "exit Finished",
            "exit Connected",
            "entry Closed",
        ]
    );
    // One microstep for the event, one for the completion.
    assert_eq!(report.microsteps, 2);
    assert_eq!(report.transitions, 2);
    // The constructor moved the data of `Connected` into the receipt.
    assert_eq!(
        machine.take_output(),
        Some(Receipt {
            session: SESSION,
            frames: 1
        })
    );
}

#[test]
fn ac_10_a_root_final_makes_the_machine_done_and_later_sends_are_ignored() {
    let mut machine = connected();
    machine.send(Event::Close).unwrap();
    assert!(machine.is_done());
    assert_eq!(machine.path().to_string(), "Closed");
    lines(&mut machine);

    let report = machine.send(Event::Poke).unwrap().report;

    assert!(!report.handled);
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert_eq!(report.microsteps, 0);
    assert!(lines(&mut machine).is_empty());
    assert!(!machine.is_poisoned());
}

#[test]
fn ac_10_a_machine_that_is_not_in_a_root_final_has_no_output() {
    let mut machine = connected();
    assert!(!machine.is_done());
    assert_eq!(machine.take_output(), None);
    assert!(matches!(
        machine.state(),
        Conn::Connected(Connected {
            link: Link::Opening,
            ..
        })
    ));
    let idle = started();
    assert_eq!(
        *idle.state(),
        Conn::Idle(Idle {
            attempts: 0,
            sub: IdleSub::Waiting
        })
    );
}

#[test]
fn ac_10_the_output_is_taken_once_and_the_machine_stays_done() {
    let mut machine = connected();
    machine.send(Event::Close).unwrap();
    assert!(machine.take_output().is_some());
    assert_eq!(machine.take_output(), None);
    assert!(machine.is_done());
    assert!(machine.try_state().is_none());
    let report = machine.send(Event::Poke).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert!(!machine.is_poisoned());
}
