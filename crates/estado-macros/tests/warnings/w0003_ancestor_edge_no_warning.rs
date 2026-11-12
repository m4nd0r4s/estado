//! A `defer!()` in a state whose ancestor can leave is fine: the event is
//! redelivered when the ancestor transitions.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Data,
    Abort,
}

#[machine(event = Ev)]
mod uploader {
    use super::*;

    #[root]
    pub enum Uploader {
        #[initial]
        Active(Active),
        Aborted,
    }

    #[state]
    pub struct Active {
        #[substates]
        phase: Phase,
    }

    #[state]
    pub enum Phase {
        #[initial]
        Waiting,
    }

    #[handlers]
    impl Uploader {
        #[on(Active)]
        fn on_active(_up: ActiveFields<'_>, _cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Abort => goto!(Aborted),
                Ev::Data => bubble!(),
            }
        }

        #[on(Waiting)]
        fn on_waiting(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Data => defer!(),
                Ev::Abort => bubble!(),
            }
        }

        #[on(Aborted)]
        fn on_aborted(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Active)
        }
    }
}

fn main() {}
