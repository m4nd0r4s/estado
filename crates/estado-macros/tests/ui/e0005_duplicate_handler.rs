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
        fn first(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            bubble!()
        }

        #[on(Idle)]
        fn second(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            stay!()
        }
    }
}

fn main() {}
