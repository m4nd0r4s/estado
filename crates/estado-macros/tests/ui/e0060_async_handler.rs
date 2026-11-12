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
        async fn idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            bubble!()
        }
    }
}

fn main() {}
