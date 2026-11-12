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
        #[on(Idle)]
        const LIMIT: u8 = 3;
    }
}

fn main() {}
