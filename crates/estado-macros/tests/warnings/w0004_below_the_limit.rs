//! No warning when every variant fits `size_warn` (here exactly at the limit),
//! and none for a variant without data.
#![deny(deprecated)]

use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, size_warn = 64)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Idle,
        AtTheLimit { buffer: [u8; 64] },
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
    }
}

fn main() {}
