//! A compound state is measured without its substates: the big leaf warns
//! once, and `Active` (small fields, big substate) does not.
#![deny(deprecated)]

use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, size_warn = 32)]
mod player { use super::*;
    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
    }

    #[state]
    pub struct Active {
        label: u8,
        #[substates]
        sub: ActiveSub,
    }

    #[state]
    pub enum ActiveSub {
        #[initial]
        Quiet,
        Loud { samples: [u8; 128] },
    }
}

fn main() {}
