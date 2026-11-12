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
        #[terminal]
        Done,
    }

    #[handlers]
    impl Player {
        #[on(Done)]
        fn done(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Idle)
        }
    }
}

fn main() {}
