use estado_core::Next;
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
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
            Self::turn_on()
        }

        // A helper has no role attribute, so it has no state to rewrite
        // the transition against.
        fn turn_on() -> Next<Self> {
            goto!(On)
        }

        fn stay_put() -> Next<Self> {
            stay!()
        }
    }
}

fn main() {}
