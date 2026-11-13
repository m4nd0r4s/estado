//! `#[initial(with = f)]` of a region (errata 15): the parent is the parallel
//! state, so `f` receives its shared fields; the initializer of every region
//! that the transition does not choose runs when the parallel state is
//! entered.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Effect, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Left(u8),
    Right(u8),
}

#[machine(event = Event, effect = Note, allow(W0001, W0002))]
mod dual {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Dual {
        #[initial]
        Off,
        On(On),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct On {
        pub label: u8,
        #[region]
        pub left: Left,
        #[region]
        pub right: Right,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Left {
        #[initial(with = left_start)]
        LeftIdle(u8),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Right {
        #[initial(with = right_start)]
        RightIdle(u8),
    }

    fn left_start<S: Sink<Note>>(cx: &mut Cx<'_, Dual, S>, fields: OnFields<'_>) -> Left {
        cx.emit(Note::Left(*fields.label));
        Left::LeftIdle(*fields.label)
    }

    /// The fields alone, the `Cx` is not needed to build the value.
    fn right_start(fields: OnFields<'_>) -> Right {
        Right::RightIdle(*fields.label + 1)
    }

    #[handlers]
    impl Dual {
        #[on(Off)]
        fn on_off(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(On { label: 4 }),
                _ => unhandled!(),
            }
        }

        #[on(On)]
        fn on_on(event: &Event) -> Next<Self> {
            match event {
                Event::Stop => goto!(Off),
                _ => unhandled!(),
            }
        }

        #[on(LeftIdle)]
        fn on_left(_event: &Event) -> Next<Self> {
            bubble!()
        }

        #[on(RightIdle)]
        fn on_right(_event: &Event) -> Next<Self> {
            bubble!()
        }
    }
}

use dual::{Dual, Left, On, Right};

#[test]
fn entering_a_parallel_state_runs_the_initializer_of_each_region() {
    let mut machine = Machine::<Dual>::new(());

    let step = machine.send(Event::Start).unwrap();

    let notes: Vec<Note> = step
        .effects
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::User(note) => Some(note),
            _ => None,
        })
        .collect();
    assert_eq!(notes, [Note::Left(4)]);
    assert_eq!(
        *machine.state(),
        Dual::On(On {
            label: 4,
            left: Left::LeftIdle(4),
            right: Right::RightIdle(5),
        })
    );
}
