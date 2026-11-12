use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Idle,
    }

    #[root]
    pub enum Recorder {
        #[initial]
        Stopped,
    }
}

fn main() {}
