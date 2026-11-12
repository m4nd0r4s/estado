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
    }

    #[handlers]
    impl Player {
        #[always(Idle)]
        fn always_idle(_cx: &mut Cx<Self>) -> Next<Self> {
            reenter!()
        }
    }
}

fn main() {}
