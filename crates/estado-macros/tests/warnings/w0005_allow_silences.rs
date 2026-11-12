//! `allow(W0005)` generates nothing, whatever the lint level.
#![deny(deprecated)]

use std::time::Duration;

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Start,
    Timeout,
}

#[machine(event = Ev, allow(W0005))]
mod client {
    use super::*;

    #[root]
    pub enum Client {
        #[initial]
        Idle,
        #[after(Duration::from_secs(3) => Ev::Timeout)]
        Connecting,
    }

    #[handlers]
    impl Client {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Connecting)
        }

        #[on(Connecting)]
        fn on_connecting(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(Idle),
                _ => bubble!(),
            }
        }
    }
}

fn main() {}
