use std::time::Duration;

use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Playing {
            pos: Duration,
        },
        Paused,
    }
}

fn main() {}
