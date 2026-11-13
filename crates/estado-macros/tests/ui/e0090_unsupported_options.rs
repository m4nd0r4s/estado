use estado_core::Next;
use estado_macros::machine;

pub struct Ev;

// `typestate`, `infallible` and `version` parse, but nothing generates code
// for them yet.
#[machine(event = Ev, typestate, infallible, version = 2)]
mod light {
    use super::*;

    #[root]
    pub enum Light {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Light {
        #[on(Off)]
        fn on_off(_event: &Ev) -> Next<Self> {
            goto!(On)
        }
    }
}

fn main() {}
