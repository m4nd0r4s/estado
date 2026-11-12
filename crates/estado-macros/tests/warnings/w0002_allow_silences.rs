//! `allow(W0002)` generates nothing, whatever the lint level.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod door {
    use super::*;

    #[root]
    pub enum Door {
        #[initial]
        Closed,
        Jammed,
    }

    #[handlers]
    impl Door {
        #[on(Closed)]
        fn on_closed(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Jammed)
        }
    }
}

fn main() {}
