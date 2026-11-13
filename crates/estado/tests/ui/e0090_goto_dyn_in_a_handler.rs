use estado::prelude::*;

pub struct Ev;

#[estado::machine(event = Ev)]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Playing,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto_dyn!(Player::Playing)
        }

        #[on(Playing)]
        fn playing(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self, edges![Idle | Playing]> {
            goto!(Idle)
        }
    }
}

fn main() {}
