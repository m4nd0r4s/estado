//! The generated runtime: the defer queue of spec 5.4 and one epoch counter per
//! state with a timer (spec 3.1, 5.3 step 8).

use estado_core::{Queues, Runtime};

use super::model::{DEFER_CAP, Event, RAISE_CAP};

/// Everything `Machine` keeps besides the state and the context.
pub struct CharacterRuntime {
    queues: Queues<Event, RAISE_CAP, DEFER_CAP>,
    pub(super) jumping_epoch: u32,
    pub(super) firing_epoch: u32,
    pub(super) reloading_epoch: u32,
}

impl Runtime for CharacterRuntime {
    type Event = Event;
    type Queues = Queues<Event, RAISE_CAP, DEFER_CAP>;
    const NEW: Self = Self {
        queues: Queues::new(),
        jumping_epoch: 0,
        firing_epoch: 0,
        reloading_epoch: 0,
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }
}

/// Starts a new state instance: the epoch moves on, wrapping (spec 3.1).
pub(super) fn next_epoch(counter: &mut u32) -> u32 {
    *counter = counter.wrapping_add(1);
    *counter
}
