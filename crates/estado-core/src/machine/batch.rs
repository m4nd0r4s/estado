//! `Machine::send_batch`: a dedicated loop over a live machine (spec 4.5).

use super::{Machine, PoisonGuard, settle_error, step_live};
use crate::error::SendError;
use crate::report::{IgnoreReason, Report};
use crate::sink::Sink;
use crate::slot::Slot;
use crate::spec::Spec;

impl<M: Spec> Machine<M> {
    /// Sends every event in order; stops at the first error. The report
    /// accumulates counters; `handled` and `ignored` describe the last event.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn send_batch<S: Sink<M::Effect>>(
        &mut self,
        events: impl IntoIterator<Item = M::Event>,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        let mut events = events.into_iter();
        let mut total = BatchTotal::default();
        // The slow path of the first event (an unstarted machine starts, a
        // poisoned one refuses, a finished one ignores) stays out of the loop.
        if !matches!(self.slot, Slot::Live(_)) {
            let Some(event) = events.next() else {
                return Ok(Report::default());
            };
            total.add(self.send_when_not_live(event, sink)?);
            if !matches!(self.slot, Slot::Live(_)) {
                // Finished: every other event is ignored the same way.
                for event in events {
                    total.add(self.send_when_not_live(event, sink)?);
                }
                return Ok(total.finish());
            }
        }
        // A live machine stays live until an error returns, so the loop
        // skips the slot test and the result of `send_into` per event.
        for event in events {
            let guard = PoisonGuard {
                slot: &mut self.slot,
            };
            let mut report = Report::default();
            let stepped = step_live(
                guard.slot,
                &mut self.ctx,
                &mut self.runtime,
                sink,
                event,
                &mut report,
            );
            if let Err(error) = stepped {
                return Err(settle_error(guard, error));
            }
            guard.disarm();
            total.add(report);
        }
        Ok(total.finish())
    }
}

/// The report of a batch: counters summed over the events, `handled` and
/// `ignored` of the last one.
///
/// The counters are wide and clamped once at the end: the same value as
/// saturating at every event (all addends are non-negative), without the
/// compare per event.
#[derive(Default)]
struct BatchTotal {
    handled: bool,
    ignored: Option<IgnoreReason>,
    transitions: u64,
    microsteps: u64,
    unhandled_internal: u64,
}

impl BatchTotal {
    #[inline(always)]
    fn add(&mut self, report: Report) {
        self.handled = report.handled;
        self.ignored = report.ignored;
        self.transitions += u64::from(report.transitions);
        self.microsteps += u64::from(report.microsteps);
        self.unhandled_internal += u64::from(report.unhandled_internal);
    }

    fn finish(self) -> Report {
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
