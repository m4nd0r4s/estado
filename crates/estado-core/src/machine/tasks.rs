//! Timer and invoke results entering the machine (spec 7.1, 7.2).

use super::Machine;
use crate::effect::{InvokeKey, TimerKey};
use crate::error::SendError;
use crate::report::{IgnoreReason, Report, Step};
use crate::sink::Sink;
use crate::slot::Slot;
use crate::spec::Spec;

impl<M: Spec> Machine<M> {
    /// Fires the timer `key`: when it belongs to the active state instance,
    /// builds its event from the current state data and processes it like
    /// [`send`](Self::send); otherwise returns `Ok` with `ignored = Stale`.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn fire(&mut self, key: TimerKey) -> Result<Step<M::Effects>, SendError<M::Event>> {
        let mut effects = M::Effects::default();
        let report = self.fire_into(key, &mut effects)?;
        Ok(Step { effects, report })
    }

    /// [`fire`](Self::fire) writing effects into `sink`.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn fire_into<S: Sink<M::Effect>>(
        &mut self,
        key: TimerKey,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        let event = match &self.slot {
            Slot::Poisoned => return Err(SendError::Poisoned),
            Slot::Unstarted | Slot::Finished => None,
            Slot::Live(state) => state.timer_event(&self.runtime, key),
        };
        match event {
            Some(event) => self.send_into(event, sink),
            None => Ok(stale()),
        }
    }

    /// Delivers the result of the invoke `key` as `event`: processed like
    /// [`send`](Self::send) when `key` belongs to the active state instance,
    /// otherwise `Ok` with `ignored = Stale` and the event is dropped.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn complete(
        &mut self,
        key: InvokeKey,
        event: M::Event,
    ) -> Result<Step<M::Effects>, SendError<M::Event>> {
        let mut effects = M::Effects::default();
        let report = self.complete_into(key, event, &mut effects)?;
        Ok(Step { effects, report })
    }

    /// [`complete`](Self::complete) writing effects into `sink`.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn complete_into<S: Sink<M::Effect>>(
        &mut self,
        key: InvokeKey,
        event: M::Event,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        let accepted = match &self.slot {
            Slot::Poisoned => return Err(SendError::Poisoned),
            Slot::Unstarted | Slot::Finished => false,
            Slot::Live(state) => state.accepts_invoke(&self.runtime, key),
        };
        if accepted {
            self.send_into(event, sink)
        } else {
            Ok(stale())
        }
    }
}

fn stale() -> Report {
    Report {
        ignored: Some(IgnoreReason::Stale),
        ..Report::default()
    }
}
