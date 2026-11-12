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
        Playing,
        Paused,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Playin)
        }
    }
}

fn main() {}
