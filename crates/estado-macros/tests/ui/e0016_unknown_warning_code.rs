use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0001, W0044, Z0001))]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Idle,
    }
}

fn main() {}
