use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        Failed {
            reason: u8,
        },
    }

    #[state]
    pub struct Active {
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
        #[on(Playing)]
        fn playing(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            // The closure builds the whole value: the data written next to
            // the target would be ignored, so it is an error.
            goto!(Failed { reason: 1 }, |old: Exiting![Active]| Player::Failed { reason: 2 })
        }
    }
}

fn main() {}
