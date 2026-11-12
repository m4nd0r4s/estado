//! A `defer!()` next to a `goto!` of the same state is fine, and so is one in
//! a state whose substate can leave.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Data,
    Ready,
}

#[machine(event = Ev)]
mod uploader {
    use super::*;

    #[root]
    pub enum Uploader {
        #[initial]
        Waiting,
        Busy(Busy),
    }

    #[state]
    pub struct Busy {
        #[substates]
        step: Step,
    }

    #[state]
    pub enum Step {
        #[initial]
        First,
        Second,
    }

    #[handlers]
    impl Uploader {
        #[on(Waiting)]
        fn on_waiting(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Data => defer!(),
                Ev::Ready => goto!(Busy(Busy { step: Step::First })),
            }
        }

        // The defer is in the compound state; its substate has the edge.
        #[on(Busy)]
        fn on_busy(_up: BusyFields<'_>, _cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Data => defer!(),
                Ev::Ready => bubble!(),
            }
        }

        #[on(First)]
        fn on_first(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Second)
        }

        #[on(Second)]
        fn on_second(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Waiting)
        }
    }
}

fn main() {}
