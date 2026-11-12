use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    pub enum Player {
        Idle,
    }
}

fn main() {}
