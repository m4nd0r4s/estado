//! ADR 0013 item 2 and amendment 3: inside a region the search for an
//! `#[unhandled(p)]` override stops at the top of the region; the parallel
//! state decides after every region looked (spec 5.6). So an `unhandled!()` in
//! a region leaf reaches the override of the parallel state, unless another
//! region consumes the event.
#![allow(dead_code, missing_docs)]

use estado_core::{Machine, Next, SendError};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Open,
    Poke,
    Poke2,
    Nudge,
}

#[machine(event = Event, allow(W0001, W0002))]
mod console {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Console {
        #[initial]
        Closed,
        Open(Open),
    }

    #[state]
    #[unhandled(error)]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Open {
        #[region]
        pub left: Left,
        #[region]
        pub right: Right,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Left {
        #[initial]
        LeftIdle,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Right {
        #[initial]
        RightIdle,
    }

    #[handlers]
    impl Console {
        #[on(Closed)]
        fn closed(event: &Event) -> Next<Self> {
            match event {
                Event::Open => goto!(Open),
                _ => unhandled!(),
            }
        }

        /// Says `unhandled!()` for what it does not know.
        #[on(LeftIdle)]
        fn left_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Nudge => stay!(),
                _ => unhandled!(),
            }
        }

        /// Says `bubble!()`: nothing above it in the region.
        #[on(RightIdle)]
        fn right_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Poke2 => stay!(),
                _ => bubble!(),
            }
        }
    }
}

use console::Console;

fn open() -> Machine<Console> {
    let mut machine = Machine::<Console>::new(());
    machine.send(Event::Open).unwrap();
    machine
}

#[test]
fn unhandled_in_a_region_leaf_reaches_the_override_of_the_parallel_state() {
    let mut machine = open();

    let result = machine.send(Event::Poke);

    assert!(matches!(result, Err(SendError::Unhandled(Event::Poke))));
}

#[test]
fn another_region_consuming_the_event_spares_the_override() {
    let mut machine = open();

    let result = machine.send(Event::Poke2);

    assert!(result.unwrap().report.handled);
}

#[test]
fn the_region_that_handles_the_event_itself_spares_the_override() {
    let mut machine = open();

    let result = machine.send(Event::Nudge);

    assert!(result.unwrap().report.handled);
}
