//! A `pub` machine in a crate that denies `missing_docs` and `unreachable_pub`:
//! the generated items must not trip either lint (they are hidden, private or
//! documented), whatever features the machine uses.
#![deny(missing_docs, unreachable_pub)]

use estado_core::Next;
use estado_macros::machine;

/// The events of the player.
#[derive(Debug)]
pub enum Event {
    /// Start playing.
    Play,
    /// Stop.
    Stop,
}

/// A player with a compound state, regions, history and timers.
#[machine(event = Event, ctx = u8, allow(W0001, W0002))]
pub mod player {
    use super::*;

    /// The root.
    #[root]
    pub enum Player {
        /// Not playing.
        #[initial]
        Idle,
        /// Playing.
        Active(Active),
    }

    /// The playing state.
    #[state]
    pub struct Active {
        /// How many rounds ran.
        pub rounds: u8,
        /// The mode.
        #[substates]
        pub mode: Mode,
    }

    /// The modes.
    #[state]
    pub enum Mode {
        /// Running.
        #[initial]
        Running,
        /// Paused.
        Paused,
    }

    #[handlers]
    impl Player {
        /// Idle starts playing.
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Play => goto!(Active { rounds: 0 }),
                Event::Stop => bubble!(),
            }
        }

        /// Running stops.
        #[on(Running)]
        fn running(up: ActiveFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Stop => {
                    *up.rounds += 1;
                    goto!(Paused)
                }
                Event::Play => bubble!(),
            }
        }
    }
}

fn main() {
    let mut machine = estado_core::Machine::<player::Player>::new(0);
    machine.send(Event::Play).unwrap();
    machine.send(Event::Stop).unwrap();
}
