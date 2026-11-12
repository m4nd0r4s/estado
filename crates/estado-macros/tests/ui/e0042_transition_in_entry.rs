use estado_core::Cx;
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
    }

    #[handlers]
    impl Player {
        #[entry(Idle)]
        fn entry_idle(_cx: &mut Cx<Self>) {
            goto!(Playing);
        }
    }
}

fn main() {}
