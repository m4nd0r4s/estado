use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, defer_capp = 4, unhandled = ignor, typestate = true)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Idle,
    }
}

fn main() {}
