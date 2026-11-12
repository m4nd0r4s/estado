use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player;

fn main() {}
