//! A machine whose states are all reachable and can all be left reports
//! neither W0001 nor W0002, regions, completion and `#[always]` included.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod gate {
    use super::*;

    #[root]
    pub enum Gate {
        #[initial]
        Idle,
        Busy(Busy),
        Checking,
        #[terminal]
        Done,
    }

    #[state]
    pub struct Busy {
        #[region]
        left: Left,
        #[region]
        right: Right,
    }

    #[state]
    pub enum Left {
        #[initial]
        Waiting,
        Working,
    }

    #[state]
    pub enum Right {
        #[initial]
        Resting,
        Running,
    }

    #[handlers]
    impl Gate {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Busy)
        }

        #[on(Waiting)]
        fn on_waiting(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Working)
        }

        #[on(Working)]
        fn on_working(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Waiting)
        }

        #[on(Resting)]
        fn on_resting(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Running)
        }

        #[on(Running)]
        fn on_running(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Resting)
        }

        #[on(Busy)]
        fn on_busy(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Checking)
        }

        #[always(Checking)]
        fn always_checking(_cx: &mut Cx<Self>) -> Next<Self> {
            goto!(Done)
        }
    }
}

fn main() {}
