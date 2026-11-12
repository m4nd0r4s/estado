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
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn idle(_event: &Ev, _cx: &mut Cx<Self>) -> Next<Self> {
            bubble!()
        }
    }
}

fn main() {}
