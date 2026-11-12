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
        Failed,
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
            // The constructor receives `Active` (the child of the LCA that
            // exits), not `Playing`.
            goto!(Failed, |old: Exiting![Playing]| Player::Failed)
        }

        #[on(Paused)]
        fn paused(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            // `Failed` is not below `Idle`.
            goto!(Idle / Failed)
        }
    }
}

fn main() {}
