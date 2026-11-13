//! What a step counts, and where: one `Report` per `send`, one running
//! `BatchTotal` per `send_batch`.

use super::Source;
use crate::report::{IgnoreReason, Outcome, Report};

/// The counters a step writes. The step code is generic over it so that
/// `send_batch` counts straight into its running total instead of building a
/// `Report` per event and adding it (ADR 0017, follow-up).
pub(super) trait Tally {
    /// Counts one microstep that ended with `outcome`.
    fn count(&mut self, outcome: Outcome, source: &Source);
    /// The external event was ignored.
    fn ignore_external(&mut self);
    /// An internal or requeued event nobody handled.
    fn unhandled_internal(&mut self);
    /// Microsteps run by the current step, for the livelock bound.
    fn microsteps_of_step(&self) -> u16;
}

impl Tally for Report {
    #[inline(always)]
    fn count(&mut self, outcome: Outcome, source: &Source) {
        self.microsteps += 1;
        if let Outcome::Transitioned { .. } = outcome {
            self.transitions += 1;
        }
        if matches!(source, Source::External) {
            self.handled = !matches!(outcome, Outcome::Unhandled(_));
        }
    }

    #[inline(always)]
    fn ignore_external(&mut self) {
        self.ignored = Some(IgnoreReason::Unhandled);
    }

    #[inline(always)]
    fn unhandled_internal(&mut self) {
        self.unhandled_internal += 1;
    }

    #[inline(always)]
    fn microsteps_of_step(&self) -> u16 {
        self.microsteps
    }
}

/// The report of a batch: counters summed over the events, `handled` and
/// `ignored` of the last one.
///
/// The counters are wide and clamped once at the end: the same value as
/// saturating at every event (all addends are non-negative), without the
/// compare per event.
#[derive(Default)]
pub(super) struct BatchTotal {
    handled: bool,
    ignored: Option<IgnoreReason>,
    transitions: u64,
    microsteps: u64,
    unhandled_internal: u64,
    /// `microsteps` when the current step began.
    step_start: u64,
}

impl BatchTotal {
    /// Starts counting a new event: `handled` and `ignored` describe the
    /// last event only.
    #[inline(always)]
    pub(super) fn begin_event(&mut self) {
        self.handled = false;
        self.ignored = None;
        self.step_start = self.microsteps;
    }

    /// Adds the report of an event that did not go through the step loop.
    #[inline(always)]
    pub(super) fn add(&mut self, report: Report) {
        self.handled = report.handled;
        self.ignored = report.ignored;
        self.transitions += u64::from(report.transitions);
        self.microsteps += u64::from(report.microsteps);
        self.unhandled_internal += u64::from(report.unhandled_internal);
    }

    pub(super) fn finish(self) -> Report {
        let clamp = |count: u64| u16::try_from(count).unwrap_or(u16::MAX);
        Report {
            handled: self.handled,
            ignored: self.ignored,
            transitions: clamp(self.transitions),
            microsteps: clamp(self.microsteps),
            unhandled_internal: clamp(self.unhandled_internal),
        }
    }
}

impl Tally for BatchTotal {
    #[inline(always)]
    fn count(&mut self, outcome: Outcome, source: &Source) {
        self.microsteps += 1;
        if let Outcome::Transitioned { .. } = outcome {
            self.transitions += 1;
        }
        if matches!(source, Source::External) {
            self.handled = !matches!(outcome, Outcome::Unhandled(_));
        }
    }

    #[inline(always)]
    fn ignore_external(&mut self) {
        self.ignored = Some(IgnoreReason::Unhandled);
    }

    #[inline(always)]
    fn unhandled_internal(&mut self) {
        self.unhandled_internal += 1;
    }

    #[inline(always)]
    fn microsteps_of_step(&self) -> u16 {
        // One step is bounded by `MAX_MICROSTEPS` (a `u16`) before the
        // livelock error, so the difference fits.
        (self.microsteps - self.step_start) as u16
    }
}
