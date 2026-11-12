//! The runtime contract (spec 7.3).

use core::time::Duration;

use crate::effect::{Effect, InvokeKey, TimerKey};
use crate::sink::{Overflow, Sink};

/// What any runtime implements to execute effects. Synchronous: async
/// runtimes spawn their work from it.
///
/// Generic over the command type, like [`Sink`] (ADR 0004).
pub trait Driver<Command> {
    /// Runs a user effect.
    fn effect(&mut self, command: Command);
    /// Schedules the timer `key` to fire after `after`.
    fn start_timer(&mut self, key: TimerKey, after: Duration);
    /// Cancels the timer `key`; firing it later is harmless (stale).
    fn cancel_timer(&mut self, key: TimerKey);
    /// Starts the task `key`.
    fn invoke(&mut self, key: InvokeKey, task: Command);
    /// Cancels the task `key`; completing it later is harmless (stale).
    fn cancel(&mut self, key: InvokeKey);
}

/// Dispatches one effect to the matching [`Driver`] method.
///
/// ```
/// use core::time::Duration;
/// use estado_core::{Driver, Effect, InvokeKey, TimerKey, drive};
///
/// #[derive(Default)]
/// struct Recorder(Vec<String>);
/// impl Driver<&'static str> for Recorder {
///     fn effect(&mut self, command: &'static str) { self.0.push(command.into()) }
///     fn start_timer(&mut self, _: TimerKey, after: Duration) { self.0.push(format!("timer {after:?}")) }
///     fn cancel_timer(&mut self, _: TimerKey) {}
///     fn invoke(&mut self, _: InvokeKey, _: &'static str) {}
///     fn cancel(&mut self, _: InvokeKey) {}
/// }
///
/// let mut recorder = Recorder::default();
/// drive(Effect::User("beep"), &mut recorder);
/// assert_eq!(recorder.0, ["beep"]);
/// ```
pub fn drive<Command>(effect: Effect<Command>, driver: &mut impl Driver<Command>) {
    match effect {
        Effect::User(command) => driver.effect(command),
        Effect::StartTimer { key, after } => driver.start_timer(key, after),
        Effect::CancelTimer(key) => driver.cancel_timer(key),
        Effect::Invoke { key, task } => driver.invoke(key, task),
        Effect::Cancel(key) => driver.cancel(key),
    }
}

/// Uses a [`Driver`] as a [`Sink`]: each effect is driven as it is emitted.
pub struct DriverSink<'driver, D>(pub &'driver mut D);

impl<Command, D: Driver<Command>> Sink<Command> for DriverSink<'_, D> {
    #[inline]
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        drive(effect, self.0);
        Ok(())
    }
}
