use std::time::Duration;

use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Playing {
            pos: Duration,
        },
        Paused {
            pos: Duration,
        },
    }

    #[handlers]
    impl Player {
        #[on(Playing)]
        fn playing(_paused: PausedMut<'_>, _cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            bubble!()
        }
    }
}

fn main() {}
