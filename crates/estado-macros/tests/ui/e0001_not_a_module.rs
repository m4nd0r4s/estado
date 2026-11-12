use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
enum Player {
    Idle,
}

fn main() {}
