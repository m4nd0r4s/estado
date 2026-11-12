//! What a `send` reports (spec 4.5).

use crate::spec::UnhandledPolicy;

/// Result of one microstep, returned by generated code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Handled without a transition.
    Stayed,
    /// Nobody handled the event; apply this policy (the machine's, or the
    /// `#[unhandled]` override of the state the event left from).
    Unhandled(UnhandledPolicy),
    /// The event must be deferred.
    Deferred,
    /// A transition ran.
    Transitioned {
        /// Whether the active configuration changed (false for a
        /// self-transition that re-entered the same states).
        changed: bool,
    },
}

/// Why an event was ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreReason {
    /// Unhandled under the `ignore` policy, or the machine is done.
    Unhandled,
    /// A timer or invoke of a state instance that already exited.
    Stale,
}

/// Counters of one `send`.
///
/// ```
/// let report = estado_core::Report::default();
/// assert!(!report.handled);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    /// The external event was handled (stay, transition or defer).
    pub handled: bool,
    /// Transitions run, all microsteps included.
    pub transitions: u16,
    /// Microsteps run, the external event included.
    pub microsteps: u16,
    /// Internal or requeued events nobody handled.
    pub unhandled_internal: u16,
    /// Set when the external event was ignored.
    pub ignored: Option<IgnoreReason>,
}

/// Effects plus report of one `send`.
#[derive(Debug)]
pub struct Step<Effects> {
    /// Effects in emission order.
    pub effects: Effects,
    /// Counters.
    pub report: Report,
}
