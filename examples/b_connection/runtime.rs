//! The generated runtime: the queues of spec 5.4 and one epoch counter per
//! state with timers or invokes (spec 3.1, 5.3 step 8).

use estado_core::{Queues, Runtime};

use super::model::{DEFER_CAP, Event, RAISE_CAP};

/// Everything `Machine` keeps besides the state and the context.
pub struct ConnRuntime {
    queues: Queues<Event, RAISE_CAP, DEFER_CAP>,
    pub(super) connecting_epoch: u32,
    pub(super) backoff_epoch: u32,
    pub(super) sending_epoch: u32,
    pub(super) pinging_epoch: u32,
    pub(super) awaiting_pong_epoch: u32,
}

impl Runtime for ConnRuntime {
    type Event = Event;
    type Queues = Queues<Event, RAISE_CAP, DEFER_CAP>;
    const NEW: Self = Self {
        queues: Queues::new(),
        connecting_epoch: 0,
        backoff_epoch: 0,
        sending_epoch: 0,
        pinging_epoch: 0,
        awaiting_pong_epoch: 0,
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
