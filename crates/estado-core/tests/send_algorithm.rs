//! Acceptance tests of the run-to-completion algorithm (spec 5.4, 5.7,
//! 5.11) on the hand-written `Probe` machine.

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};

use estado_core::{IgnoreReason, Machine, SendError};
use support::probe::{Event, Log, Probe};

fn started() -> Machine<Probe> {
    let mut machine = Machine::<Probe>::new(Log::default());
    machine.start().expect("start");
    machine.ctx_mut().lines.clear();
    machine
}

fn lines(machine: &mut Machine<Probe>) -> Vec<String> {
    std::mem::take(&mut machine.ctx_mut().lines)
}

#[test]
fn send_on_an_unstarted_machine_starts_it_first() {
    let mut machine = Machine::<Probe>::new(Log::default());
    assert!(machine.try_state().is_none());
    machine.send(Event::Note(0)).unwrap();
    assert_eq!(
        lines(&mut machine),
        ["entry A", "A handles Note(0)", "note 0"]
    );
}

#[test]
fn ac_03_internal_events_run_after_the_external_event_in_fifo_order() {
    let mut machine = started();
    let report = machine.send(Event::RaiseTwice).unwrap().report;
    assert_eq!(
        lines(&mut machine),
        [
            "A handles RaiseTwice",
            "A handles Note(1)",
            "note 1",
            "A handles Note(2)",
            "note 2"
        ]
    );
    assert_eq!(report.microsteps, 3);
    assert!(report.handled);
}

#[test]
fn ac_03_always_runs_before_internal_events() {
    let mut machine = started();
    let report = machine.send(Event::Go(Probe::C)).unwrap().report;
    assert_eq!(
        lines(&mut machine),
        [
            "A handles Go(C)",
            "exit A",
            "entry C",
            "always C",
            "exit C",
            "entry A",
            "A handles Note(9)",
            "note 9"
        ]
    );
    assert_eq!(report.transitions, 2);
    assert_eq!(*machine.state(), Probe::A);
}

#[test]
fn ac_03_deferred_events_are_redelivered_only_after_a_configuration_change() {
    let mut machine = started();
    assert!(machine.send(Event::Hold).unwrap().report.handled);
    assert_eq!(machine.runtime().queues.deferred.len(), 1);

    machine.send(Event::RaiseTwice).unwrap();
    assert!(
        !lines(&mut machine)
            .iter()
            .any(|line| line == "held handled")
    );

    machine.send(Event::Go(Probe::B)).unwrap();
    assert_eq!(
        lines(&mut machine),
        [
            "A handles Go(B)",
            "exit A",
            "entry B",
            "B handles Hold",
            "held handled"
        ]
    );
    assert_eq!(machine.runtime().queues.deferred.len(), 0);
}

#[test]
fn ac_03_internal_events_run_before_requeued_events() {
    let mut machine = started();
    machine.send(Event::Hold).unwrap();
    machine.send(Event::Go(Probe::B)).unwrap();
    machine.send(Event::Go(Probe::A)).unwrap();
    machine.send(Event::Hold).unwrap();
    lines(&mut machine);
    // A -> C: always (C -> A) first, then the raised Note(9), then the
    // requeued Hold, which A defers again.
    machine.send(Event::Go(Probe::C)).unwrap();
    let log = lines(&mut machine);
    let position = |wanted: &str| log.iter().position(|line| line == wanted).unwrap();
    assert!(position("always C") < position("note 9"));
    assert!(position("note 9") < position("A handles Hold"));
    assert_eq!(machine.runtime().queues.deferred.len(), 1);
}

#[test]
fn defer_overflow_under_the_error_policy_gives_the_event_back() {
    let mut machine = started();
    machine.send(Event::Hold).unwrap();
    machine.send(Event::Hold).unwrap();
    assert_eq!(
        machine.send(Event::Hold).unwrap_err(),
        SendError::DeferOverflow(Event::Hold)
    );
    assert!(!machine.is_poisoned());
}

#[test]
fn ac_06_panic_in_a_handler_poisons_the_machine() {
    let mut machine = started();
    let result = catch_unwind(AssertUnwindSafe(|| machine.send(Event::Panic)));
    assert!(result.is_err());
    assert!(machine.is_poisoned());
    assert!(machine.try_state().is_none());
    assert_eq!(
        machine.send(Event::Note(1)).unwrap_err(),
        SendError::Poisoned
    );
}

#[test]
fn ac_06_panic_in_an_exit_poisons_the_machine() {
    let mut machine = started();
    let result = catch_unwind(AssertUnwindSafe(|| machine.send(Event::PanicOnExit)));
    assert!(result.is_err());
    assert!(machine.is_poisoned());
    assert_eq!(
        machine.send(Event::Note(1)).unwrap_err(),
        SendError::Poisoned
    );
}

#[test]
#[should_panic(expected = "is poisoned")]
fn ac_06_state_of_a_poisoned_machine_panics_with_a_clear_message() {
    let mut machine = started();
    let _ = catch_unwind(AssertUnwindSafe(|| machine.send(Event::Panic)));
    let _ = machine.state();
}

#[test]
fn ac_07_livelock_poisons_after_max_microsteps() {
    let mut machine = started();
    match machine.send(Event::Spin).unwrap_err() {
        SendError::Livelock { microsteps, last } => {
            assert_eq!(microsteps, 17);
            assert_eq!(last.to_string(), "A");
        }
        other => panic!("expected a livelock, got {other:?}"),
    }
    assert!(machine.is_poisoned());
}

#[test]
fn ac_08_external_unhandled_event_is_ignored_by_default() {
    let mut machine = started();
    let report = machine.send(Event::Unknown).unwrap().report;
    assert!(!report.handled);
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
}

#[test]
fn ac_08_state_override_returns_the_event_as_an_error() {
    let mut machine = started();
    machine.send(Event::Go(Probe::B)).unwrap();
    assert_eq!(
        machine.send(Event::Unknown).unwrap_err(),
        SendError::Unhandled(Event::Unknown)
    );
    assert!(!machine.is_poisoned());
    assert_eq!(*machine.state(), Probe::B);
}

#[test]
fn ac_08_internal_unhandled_events_are_counted_never_errors() {
    let mut machine = started();
    machine.send(Event::Go(Probe::B)).unwrap();
    let report = machine.send(Event::RaiseUnknown).unwrap().report;
    assert_eq!(report.unhandled_internal, 1);
    assert!(report.handled);
}

#[test]
fn send_with_and_send_batch_feed_every_event() {
    let mut machine = started();
    let mut count = 0;
    machine.send_with(Event::Note(1), |_| count += 1).unwrap();
    let report = machine
        .send_batch(
            [Event::Note(2), Event::Go(Probe::B)],
            &mut estado_core::Discard,
        )
        .unwrap();
    assert_eq!(report.microsteps, 2);
    assert_eq!(report.transitions, 1);
    assert_eq!(*machine.state(), Probe::B);
}
