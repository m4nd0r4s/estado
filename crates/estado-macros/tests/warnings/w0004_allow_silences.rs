//! `allow(W0004)` generates nothing, so `deny(deprecated)` passes.
#![deny(deprecated)]

use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, size_warn = 32, allow(W0004))]
mod player { use super::*;
    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Big { buffer: [u8; 64] },
    }
}

fn main() {}
