//! Errors of `send` (spec 4.6).

use core::fmt;

use crate::ids::StatePath;

/// Why a `send` failed.
///
/// ```
/// use estado_core::SendError;
/// let error = SendError::Unhandled(7u8);
/// assert_eq!(error.to_string(), "no state handled the event");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError<Event> {
    /// A previous panic or fatal error poisoned the machine.
    Poisoned,
    /// Unhandled under the `error` policy; the event is given back and nothing changed.
    Unhandled(Event),
    /// The defer queue is full under the `error` overflow policy; the event
    /// that did not fit is given back.
    ///
    /// When the external event itself overflows, nothing of the step ran: the
    /// machine is unchanged and not poisoned. When a raised or redelivered
    /// event overflows, the microsteps before it already ran and their effects
    /// went out, so the step is cut half-way and the machine is poisoned, like
    /// every other fault that stops a step half-way.
    DeferOverflow(Event),
    /// The internal queue overflowed; the machine is poisoned.
    RaiseOverflow,
    /// The effect sink refused an effect; the machine is poisoned.
    ///
    /// The microstep that emitted the effect had already run: its transition
    /// is applied, the refused effect is lost, and the rest of the step
    /// (raised events, `#[always]`, redelivery) never runs. Effects such as
    /// timers and tasks are how the machine tells the driver what to start and
    /// cancel, so a lost one leaves the driver out of step with the machine.
    /// That is why every `EffectOverflow` poisons, during a `start` and during
    /// a `send` alike. Size `fx_inline` for the largest burst, or use a sink
    /// that cannot overflow.
    EffectOverflow,
    /// More than `max_microsteps` microsteps; the machine is poisoned.
    Livelock {
        /// Microsteps run.
        microsteps: u16,
        /// Active configuration when it stopped.
        last: StatePath,
    },
}

impl<Event> fmt::Display for SendError<Event> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Poisoned => {
                formatter.write_str("the machine is poisoned by an earlier panic or fatal error")
            }
            Self::Unhandled(_) => formatter.write_str("no state handled the event"),
            Self::DeferOverflow(_) => formatter.write_str("the defer queue is full"),
            Self::RaiseOverflow => {
                formatter.write_str("the internal event queue overflowed; the machine is poisoned")
            }
            Self::EffectOverflow => {
                formatter.write_str("the effect buffer is full; the machine is poisoned")
            }
            Self::Livelock { microsteps, last } => write!(
                formatter,
                "livelock: {microsteps} microsteps without settling, last in {last}; the machine is poisoned"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl<Event: fmt::Debug> std::error::Error for SendError<Event> {}
