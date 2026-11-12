use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Idle,
        #[initial]
        Busy,
    }
}

fn main() {}
