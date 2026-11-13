//! `send_batch` does what a loop of `send_into` does, report included
//! (spec 4.5), on the hand-written `Probe` machine.

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};

use estado_core::{Discard, IgnoreReason, Machine, Report, SendError};
use support::probe::{Event, Log, Probe};

/// The merge `send_batch` documents, written the slow way over the reports
/// of single sends.
fn merged(reports: &[Report]) -> Report {
    let mut total = Report::default();
    for report in reports {
        total.handled = report.handled;
        total.ignored = report.ignored;
        total.transitions += report.transitions;
        total.microsteps += report.microsteps;
        total.unhandled_internal += report.unhandled_internal;
    }
    total
}

fn session() -> Vec<Event> {
    vec![
        Event::Note(1),
        Event::RaiseTwice,
        Event::Go(Probe::B),
        Event::RaiseUnknown,
        Event::Go(Probe::C),
        Event::Note(2),
    ]
}

#[test]
fn a_batch_reports_the_sum_of_its_events() {
    let mut looped = Machine::<Probe>::new(Log::default());
    let reports: Vec<Report> = session()
        .into_iter()
        .map(|event| looped.send_into(event, &mut Discard).unwrap())
        .collect();
    let mut batched = Machine::<Probe>::new(Log::default());
    let report = batched.send_batch(session(), &mut Discard).unwrap();
    assert_eq!(report, merged(&reports));
    assert_eq!(report.unhandled_internal, 1);
    assert_eq!(batched.ctx().lines, looped.ctx().lines);
    assert_eq!(batched.state(), looped.state());
}

#[test]
fn an_empty_batch_does_not_start_the_machine() {
    let mut machine = Machine::<Probe>::new(Log::default());
    let report = machine.send_batch(Vec::new(), &mut Discard).unwrap();
    assert_eq!(report, Report::default());
    assert!(machine.try_state().is_none());
}

#[test]
fn a_batch_starts_an_unstarted_machine_before_its_first_event() {
    let mut machine = Machine::<Probe>::new(Log::default());
    machine
        .send_batch([Event::Note(1), Event::Note(2)], &mut Discard)
        .unwrap();
    assert_eq!(
        machine.ctx().lines,
        [
            "entry A",
            "A handles Note(1)",
            "note 1",
            "A handles Note(2)",
            "note 2"
        ]
    );
}

#[test]
fn a_batch_stops_at_the_first_error_and_keeps_the_machine() {
    let mut machine = Machine::<Probe>::new(Log::default());
    machine.start().unwrap();
    let error = machine
        .send_batch(
            [Event::Go(Probe::B), Event::Unknown, Event::Note(7)],
            &mut Discard,
        )
        .unwrap_err();
    assert!(matches!(error, SendError::Unhandled(Event::Unknown)));
    assert!(!machine.is_poisoned());
    assert_eq!(*machine.state(), Probe::B);
    assert!(!machine.ctx().lines.iter().any(|line| line == "note 7"));
}

#[test]
fn a_panic_in_a_batch_poisons_and_later_batches_are_refused() {
    let mut machine = Machine::<Probe>::new(Log::default());
    machine.start().unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        machine.send_batch([Event::Note(1), Event::Panic, Event::Note(2)], &mut Discard)
    }));
    assert!(result.is_err());
    assert!(machine.is_poisoned());
    assert!(matches!(
        machine.send_batch([Event::Note(3)], &mut Discard),
        Err(SendError::Poisoned)
    ));
}

#[test]
fn a_fatal_error_in_a_batch_poisons() {
    let mut machine = Machine::<Probe>::new(Log::default());
    machine.start().unwrap();
    let error = machine
        .send_batch([Event::Note(1), Event::Spin], &mut Discard)
        .unwrap_err();
    assert!(matches!(error, SendError::Livelock { .. }));
    assert!(machine.is_poisoned());
}

// 65 545 events, each logging two strings: minutes under miri.
#[test]
#[cfg_attr(miri, ignore)]
fn a_batch_counts_wide_and_clamps_once() {
    let mut machine = Machine::<Probe>::new(Log::default());
    let events = std::iter::repeat_n(Event::Note(0), usize::from(u16::MAX) + 10);
    let report = machine.send_batch(events, &mut Discard).unwrap();
    assert_eq!(report.microsteps, u16::MAX);
    assert_eq!(report.transitions, 0);
    assert!(report.handled);
    assert_eq!(report.ignored, None::<IgnoreReason>);
}
