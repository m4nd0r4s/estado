//! Storage of the root state with the started and poisoned markers
//! (ADR 0002, errata 2 and 3).

/// The root state of a machine, or why there is none.
///
/// `Live` is the only variant user code ever sees through
/// [`Machine::state`](crate::Machine::state). When the root enum has two spare
/// niche values, `Slot<M>` is exactly as big as `M`.
///
/// ```
/// use estado_core::Slot;
/// let mut slot = Slot::Live(String::from("Idle"));
/// let taken = slot.take();
/// assert!(slot.is_poisoned());
/// slot.put(taken);
/// assert_eq!(slot.live().map(String::as_str), Some("Idle"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot<State> {
    /// `Machine::new` ran, `start` did not.
    Unstarted,
    /// Running.
    Live(State),
    /// A panic or fatal error happened in the middle of a step.
    Poisoned,
}

impl<State> Slot<State> {
    /// The state, when live.
    #[inline]
    pub fn live(&self) -> Option<&State> {
        match self {
            Self::Live(state) => Some(state),
            _ => None,
        }
    }

    /// The state, mutably.
    ///
    /// # Panics
    ///
    /// If the slot is not live; generated code only calls it on a live slot.
    #[inline]
    pub fn live_mut(&mut self) -> &mut State {
        match self {
            Self::Live(state) => state,
            _ => panic!("estado: the state is not live"),
        }
    }

    /// Takes the state by value, leaving the slot poisoned until [`put`](Self::put)
    /// (spec 5.3, steps 4 to 6).
    ///
    /// # Panics
    ///
    /// If the slot is not live.
    #[inline]
    pub fn take(&mut self) -> State {
        match core::mem::replace(self, Self::Poisoned) {
            Self::Live(state) => state,
            _ => panic!("estado: the state is not live"),
        }
    }

    /// Writes the new state.
    #[inline]
    pub fn put(&mut self, state: State) {
        *self = Self::Live(state);
    }

    /// Whether a panic or fatal error poisoned the machine.
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        matches!(self, Self::Poisoned)
    }
}
