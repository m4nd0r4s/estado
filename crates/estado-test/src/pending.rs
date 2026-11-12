//! Timers and invokes scheduled by the machine and not yet resolved.

use core::time::Duration;

use estado_core::{InvokeKey, TimerKey};

/// A timer waiting on the virtual clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTimer {
    /// Its key.
    pub key: TimerKey,
    /// Virtual time at which it fires.
    pub deadline: Duration,
    /// Scheduling order, to break deadline ties.
    pub sequence: u64,
}

/// A task started by the machine and not completed yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInvoke<Command> {
    /// Its key.
    pub key: InvokeKey,
    /// The task.
    pub task: Command,
}

/// The in-memory runtime state: what the machine asked for.
#[derive(Debug)]
pub(crate) struct Pending<Command> {
    pub now: Duration,
    pub timers: Vec<PendingTimer>,
    pub invokes: Vec<PendingInvoke<Command>>,
    next_sequence: u64,
}

impl<Command> Pending<Command> {
    pub fn new() -> Self {
        Self {
            now: Duration::ZERO,
            timers: Vec::new(),
            invokes: Vec::new(),
            next_sequence: 0,
        }
    }

    pub fn start_timer(&mut self, key: TimerKey, after: Duration) {
        self.timers.retain(|timer| timer.key != key);
        self.timers.push(PendingTimer {
            key,
            deadline: self.now + after,
            sequence: self.next_sequence,
        });
        self.next_sequence += 1;
    }

    pub fn cancel_timer(&mut self, key: TimerKey) {
        self.timers.retain(|timer| timer.key != key);
    }

    pub fn cancel_invoke(&mut self, key: InvokeKey) {
        self.invokes.retain(|invoke| invoke.key != key);
    }

    /// Removes the next timer due at or before `limit`: earliest deadline,
    /// then earliest scheduled.
    pub fn take_due_timer(&mut self, limit: Duration) -> Option<PendingTimer> {
        let position = self
            .timers
            .iter()
            .enumerate()
            .filter(|(_, timer)| timer.deadline <= limit)
            .min_by_key(|(_, timer)| (timer.deadline, timer.sequence))
            .map(|(position, _)| position)?;
        Some(self.timers.remove(position))
    }
}
