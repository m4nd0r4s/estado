//! W0002 is reported for a leaf that no handler of its own or of an ancestor
//! can leave, unless it is `#[terminal]`.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod door {
    use super::*;

    #[root]
    pub enum Door {
        #[initial]
        Closed,
        Open,
        Jammed,
        #[terminal]
        Removed,
    }

    #[handlers]
    impl Door {
        #[on(Closed)]
        fn on_closed(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Open)
        }

        #[on(Open)]
        fn on_open(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            if cfg!(test) { goto!(Removed) } else { goto!(Jammed) }
        }
    }
}

fn main() {}
