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
        Active(Active),
    }

    #[state]
    pub struct Active {
        track: u32,
        volume: u8,
        #[substates]
        sub: ActiveSub,
    }

    #[state]
    pub enum ActiveSub {
        #[initial]
        Playing,
        Paused,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Active { track: 1 })
        }
    }
}

fn main() {}
