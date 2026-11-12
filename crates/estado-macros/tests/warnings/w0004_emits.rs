//! W0004 is reported for a variant larger than `size_warn`, at the variant.
#![deny(deprecated)]

use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, size_warn = 32, allow(W0001, W0002))]
mod player { use super::*;
    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Small(u32),
        Big { buffer: [u8; 64], length: usize },
        Tuple([u8; 40], u8),
        Wrapped(Wrapper),
    }

    pub struct Wrapper {
        bytes: [u8; 100],
    }
}

fn main() {}
