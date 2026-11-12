//! `allow(W0001)` generates nothing, whatever the lint level.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0001, W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Orphan,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            stay!()
        }
    }
}

fn main() {}
