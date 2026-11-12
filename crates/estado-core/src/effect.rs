//! Effects produced by a machine and the keys of timers and invokes (spec 4.4).

use core::time::Duration;

use crate::ids::StateId;

/// Something the machine asks its runtime to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect<Command> {
    /// A user effect, emitted with `cx.emit`.
    User(Command),
    /// Start a timer owned by a state instance.
    StartTimer {
        /// Identifies the timer and its state instance.
        key: TimerKey,
        /// Delay from now.
        after: Duration,
    },
    /// Cancel a timer whose state instance exited.
    CancelTimer(TimerKey),
    /// Start a task owned by a state instance.
    Invoke {
        /// Identifies the task and its state instance.
        key: InvokeKey,
        /// The task, as a user effect.
        task: Command,
    },
    /// Cancel a task whose state instance exited.
    Cancel(InvokeKey),
}

impl<Command> Effect<Command> {
    /// Converts the user payload, keeping keys (used for child machines).
    pub fn map<Other>(self, convert: impl FnOnce(Command) -> Other) -> Effect<Other> {
        match self {
            Self::User(command) => Effect::User(convert(command)),
            Self::StartTimer { key, after } => Effect::StartTimer { key, after },
            Self::CancelTimer(key) => Effect::CancelTimer(key),
            Self::Invoke { key, task } => Effect::Invoke {
                key,
                task: convert(task),
            },
            Self::Cancel(key) => Effect::Cancel(key),
        }
    }
}

/// Path from a machine down to one of its child machines (spec 7.4).
///
/// Each segment is the child field index followed, for `#[children]`, by the
/// index in the `Vec`; at most [`ChildPath::MAX_DEPTH`] segments.
///
/// ```
/// use estado_core::ChildPath;
/// let path = ChildPath::ROOT.child(1).unwrap().child(4).unwrap();
/// assert_eq!(path.segments(), &[1, 4]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChildPath {
    segments: [u16; 4],
    length: u8,
}

impl ChildPath {
    /// Maximum nesting of child machines.
    pub const MAX_DEPTH: usize = 4;

    /// The machine itself.
    pub const ROOT: Self = Self {
        segments: [0; 4],
        length: 0,
    };

    /// Appends a segment; `None` when the path is already at maximum depth.
    pub const fn child(mut self, segment: u16) -> Option<Self> {
        if self.length as usize == Self::MAX_DEPTH {
            return None;
        }
        self.segments[self.length as usize] = segment;
        self.length += 1;
        Some(self)
    }

    /// The segments, outermost first.
    pub fn segments(&self) -> &[u16] {
        &self.segments[..usize::from(self.length)]
    }
}

/// Identifies one timer of one state instance.
///
/// ```
/// use estado_core::{ChildPath, StateId, TimerKey};
/// let key = TimerKey { state: StateId::new(2), epoch: 7, index: 0, child: ChildPath::ROOT };
/// assert_eq!(key, key.clone());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimerKey {
    /// Owning state.
    pub state: StateId,
    /// Epoch of the state instance that scheduled it.
    pub epoch: u32,
    /// Which `#[after]` of that state.
    pub index: u8,
    /// Child machine that owns the state, `ROOT` for this machine.
    pub child: ChildPath,
}

/// Identifies one task of one state instance; stable and usable as an
/// idempotency key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InvokeKey {
    /// Owning state.
    pub state: StateId,
    /// Epoch of the state instance that started it.
    pub epoch: u32,
    /// Which `#[invoke]` of that state.
    pub index: u8,
    /// Child machine that owns the state, `ROOT` for this machine.
    pub child: ChildPath,
}
