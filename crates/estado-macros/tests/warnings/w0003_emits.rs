//! W0003 is reported for a `defer!()` in a state that nothing can leave: no
//! `goto!` in the state, its ancestors, its substates or beside it.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Start,
    Data,
}

// W0002 is allowed so the snapshot shows W0003 alone.
#[machine(event = Ev, allow(W0002))]
mod uploader {
    use super::*;

    #[root]
    pub enum Uploader {
        #[initial]
        Idle,
        Stuck,
    }

    #[handlers]
    impl Uploader {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Stuck)
        }

        #[on(Stuck)]
        fn on_stuck(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Data => defer!(),
                Ev::Start => stay!(),
            }
        }
    }
}

fn main() {}
