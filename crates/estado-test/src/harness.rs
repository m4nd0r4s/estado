//! The [`Harness`]: a machine driven by a virtual clock.

use core::fmt::Debug;
use core::time::Duration;

use estado_core::{Effect, InvokeKey, Machine, Overflow, Report, SendError, Sink, Spec};

use crate::pending::{Pending, PendingInvoke, PendingTimer};

/// Runs a [`Machine`] with an in-memory driver and a virtual clock that
/// starts at zero. Every method panics with a clear message on a
/// [`SendError`]; use [`machine_mut`](Self::machine_mut) to test errors.
pub struct Harness<M: Spec> {
    machine: Machine<M>,
    pending: Pending<M::Effect>,
    last_effects: Vec<M::Effect>,
    last_report: Report,
}

/// The user effects and report of the last operation, for chained asserts.
pub struct StepView<'harness, Command> {
    /// User effects emitted, in order.
    pub effects: &'harness [Command],
    /// Counters of the operation.
    pub report: Report,
}

impl<Command: Debug> StepView<'_, Command> {
    /// Asserts that some emitted user effect matches `predicate`.
    ///
    /// # Panics
    ///
    /// When none does.
    #[track_caller]
    pub fn expect_effect(&self, predicate: impl Fn(&Command) -> bool) -> &Self {
        assert!(
            self.effects.iter().any(predicate),
            "no emitted effect matches; emitted: {:?}",
            self.effects
        );
        self
    }

    /// Asserts that no user effect was emitted.
    ///
    /// # Panics
    ///
    /// When some was.
    #[track_caller]
    pub fn expect_no_effect(&self) -> &Self {
        assert!(
            self.effects.is_empty(),
            "unexpected effects: {:?}",
            self.effects
        );
        self
    }
}

impl<M: Spec> Harness<M>
where
    M::Effect: Debug,
    M::Event: Debug,
{
    /// Wraps a machine; it is started on the first operation.
    pub fn new(machine: Machine<M>) -> Self {
        Self {
            machine,
            pending: Pending::new(),
            last_effects: Vec::new(),
            last_report: Report::default(),
        }
    }

    /// Starts the machine (runs the initial entries).
    #[track_caller]
    pub fn start(&mut self) -> StepView<'_, M::Effect> {
        let result = {
            let (machine, mut sink) = self.split();
            machine.start_into(&mut sink)
        };
        self.settle(result)
    }

    /// Sends an event.
    #[track_caller]
    pub fn send(&mut self, event: M::Event) -> StepView<'_, M::Effect> {
        let result = {
            let (machine, mut sink) = self.split();
            machine.send_into(event, &mut sink)
        };
        self.settle(result)
    }

    /// Moves the virtual clock forward by `duration`, firing every due timer
    /// in deadline order (ties: scheduling order). Effects of all fired
    /// timers are collected in the returned view.
    #[track_caller]
    pub fn advance(&mut self, duration: Duration) -> StepView<'_, M::Effect> {
        let limit = self.pending.now + duration;
        let mut effects = Vec::new();
        let mut total = Report::default();
        while let Some(timer) = self.pending.take_due_timer(limit) {
            self.pending.now = timer.deadline;
            let result = {
                let (machine, mut sink) = self.split();
                machine.fire_into(timer.key, &mut sink)
            };
            self.settle(result);
            effects.append(&mut self.last_effects);
            total.microsteps += self.last_report.microsteps;
            total.transitions += self.last_report.transitions;
        }
        self.pending.now = limit;
        self.last_effects = effects;
        self.last_report = total;
        self.view()
    }

    /// Completes the oldest pending invoke whose task matches `predicate`,
    /// delivering `event`.
    ///
    /// # Panics
    ///
    /// When no pending invoke matches.
    #[track_caller]
    pub fn complete_invoke(
        &mut self,
        predicate: impl Fn(&M::Effect) -> bool,
        event: M::Event,
    ) -> StepView<'_, M::Effect> {
        let position = self
            .pending
            .invokes
            .iter()
            .position(|invoke| predicate(&invoke.task))
            .unwrap_or_else(|| {
                panic!(
                    "no pending invoke matches; pending: {:?}",
                    self.pending.invokes
                )
            });
        let invoke = self.pending.invokes.remove(position);
        self.complete_key(invoke.key, event)
    }

    /// Delivers `event` as the result of the invoke `key`, pending or not
    /// (to test stale results).
    #[track_caller]
    pub fn complete_key(&mut self, key: InvokeKey, event: M::Event) -> StepView<'_, M::Effect> {
        let result = {
            let (machine, mut sink) = self.split();
            machine.complete_into(key, event, &mut sink)
        };
        self.settle(result)
    }

    /// Current virtual time.
    pub fn now(&self) -> Duration {
        self.pending.now
    }

    /// Timers scheduled and neither fired nor cancelled.
    pub fn pending_timers(&self) -> &[PendingTimer] {
        &self.pending.timers
    }

    /// Invokes started and neither completed nor cancelled.
    pub fn pending_invokes(&self) -> &[PendingInvoke<M::Effect>] {
        &self.pending.invokes
    }

    /// Asserts there is no pending timer.
    ///
    /// # Panics
    ///
    /// When there is one.
    #[track_caller]
    pub fn assert_no_pending_timers(&self) {
        assert!(
            self.pending.timers.is_empty(),
            "pending timers: {:?}",
            self.pending.timers
        );
    }

    /// The machine.
    pub fn machine(&self) -> &Machine<M> {
        &self.machine
    }

    /// The machine, mutably (bypasses the driver).
    pub fn machine_mut(&mut self) -> &mut Machine<M> {
        &mut self.machine
    }

    /// The last operation's effects and report.
    pub fn view(&self) -> StepView<'_, M::Effect> {
        StepView {
            effects: &self.last_effects,
            report: self.last_report,
        }
    }

    /// The machine and a sink that routes effects to the pending sets and
    /// the user effect list.
    fn split(&mut self) -> (&mut Machine<M>, RecordingSink<'_, M::Effect>) {
        self.last_effects.clear();
        let sink = RecordingSink {
            pending: &mut self.pending,
            effects: &mut self.last_effects,
        };
        (&mut self.machine, sink)
    }

    #[track_caller]
    fn settle(&mut self, result: Result<Report, SendError<M::Event>>) -> StepView<'_, M::Effect> {
        match result {
            Ok(report) => self.last_report = report,
            Err(error) => panic!("machine `{}` failed: {error} ({error:?})", M::NAME),
        }
        self.view()
    }
}

/// The in-memory driver: user effects go to a list, timers and invokes to
/// the pending sets.
struct RecordingSink<'harness, Command> {
    pending: &'harness mut Pending<Command>,
    effects: &'harness mut Vec<Command>,
}

impl<Command> Sink<Command> for RecordingSink<'_, Command> {
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        match effect {
            Effect::User(command) => self.effects.push(command),
            Effect::StartTimer { key, after } => self.pending.start_timer(key, after),
            Effect::CancelTimer(key) => self.pending.cancel_timer(key),
            Effect::Invoke { key, task } => self.pending.invokes.push(PendingInvoke { key, task }),
            Effect::Cancel(key) => self.pending.cancel_invoke(key),
        }
        Ok(())
    }
}
