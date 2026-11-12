//! Errors of `send` (spec 4.6).

use core::fmt;

use crate::ids::StatePath;

/// Why a `send` failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError<Event> {
    /// A previous panic or fatal error poisoned the machine.
    Poisoned,
    /// Unhandled under the `error` policy; the event is given back and nothing changed.
    Unhandled(Event),
    /// The defer queue is full under the `error` overflow policy.
    DeferOverflow(Event),
    /// The internal queue overflowed; the machine is poisoned.
    RaiseOverflow,
    /// The effect sink is full.
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
            Self::EffectOverflow => formatter.write_str("the effect buffer is full"),
            Self::Livelock { microsteps, last } => write!(
                formatter,
                "livelock: {microsteps} microsteps without settling, last in {last}; the machine is poisoned"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl<Event: fmt::Debug> std::error::Error for SendError<Event> {}
