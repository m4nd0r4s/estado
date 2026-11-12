//! W0005 is reported for an `#[after]` whose event variant no handler names
//! and no catch-all arm handles.
#![deny(deprecated)]

use std::time::Duration;

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Start,
    Timeout,
    Retry(u8),
}

#[machine(event = Ev)]
mod client {
    use super::*;

    #[root]
    pub enum Client {
        #[initial]
        Idle,
        #[after(Duration::from_secs(3) => Ev::Timeout)]
        Connecting,
        #[after(Duration::from_secs(1) => Ev::Retry(1))]
        Waiting,
    }

    #[handlers]
    impl Client {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(Connecting),
                _ => bubble!(),
            }
        }

        // Matches `Start`, but not the `Timeout` the timer sends.
        #[on(Connecting)]
        fn on_connecting(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(Waiting),
                _ => unhandled!(),
            }
        }

        #[on(Waiting)]
        fn on_waiting(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(Idle),
                Ev::Timeout => goto!(Idle),
                _ => bubble!(),
            }
        }
    }
}

fn main() {}
