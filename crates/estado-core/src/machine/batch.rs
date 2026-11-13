//! `Machine::send_batch`: a dedicated loop over a live machine (spec 4.5).

use super::step::step_live;
use super::tally::BatchTotal;
use super::{Machine, PoisonGuard, settle_error};
use crate::error::SendError;
use crate::report::Report;
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
            total.begin_event();
            let stepped = step_live(
                guard.slot,
                &mut self.ctx,
                &mut self.runtime,
                sink,
                event,
                &mut total,
            );
            if let Err(error) = stepped {
                return Err(settle_error(guard, error));
            }
            guard.disarm();
        }
        Ok(total.finish())
    }
}
