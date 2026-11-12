use estado_core::Next;
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod bus {
    use super::*;

    #[root]
    pub enum Bus {
        #[initial]
        Idle,
        IO,
    }

    #[handlers]
    impl Bus {
        #[on(Idle)]
        fn on_idle(_event: &Ev) -> Next<Self> {
            goto!(IO)
        }
    }
}

fn main() {}
