//! Entering one region by an explicit path starts the other regions in their
//! initial states: `Right` is reachable although no edge targets it.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Go,
}

#[machine(event = Ev, allow(W0002))]
mod deep {
    use super::*;

    #[root]
    pub enum Deep {
        #[initial]
        Idle,
        Alive(Alive),
    }

    #[state]
    pub struct Alive {
        #[region]
        left: Left,
        #[region]
        right: Right,
    }

    #[state]
    pub enum Left {
        #[initial]
        L1,
        L2,
    }

    #[state]
    pub enum Right {
        #[initial]
        R1,
        R2,
    }

    #[handlers]
    impl Deep {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(L2)
        }

        #[on(L2)]
        fn on_l2(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(L1)
        }

        #[on(R1)]
        fn on_r1(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(R2)
        }
    }
}

fn main() {}
