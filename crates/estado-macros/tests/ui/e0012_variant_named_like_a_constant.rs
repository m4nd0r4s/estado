use estado_core::Next;
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod bus {
    use super::*;

    #[root]
    #[allow(non_camel_case_types)]
    pub enum Bus {
        #[initial]
        Idle,
        // The id constant of `Idle` is `Bus::IDLE_ID`.
        IDLE_ID,
    }

    #[handlers]
    impl Bus {
        #[on(Idle)]
        fn on_idle(_event: &Ev) -> Next<Self> {
            goto!(IDLE_ID)
        }
    }
}

fn main() {}
