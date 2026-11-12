//! A `defer!()` in one region is fine when another region of the same
//! parallel state can transition: that changes the configuration.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Data,
    Flip,
}

// W0002 is allowed: it looks at one leaf at a time, `Buffering` has no edge.
#[machine(event = Ev, allow(W0002))]
mod pipeline {
    use super::*;

    #[root]
    pub enum Pipeline {
        #[initial]
        Running(Running),
    }

    #[state]
    pub struct Running {
        #[region]
        input: Input,
        #[region]
        mode: Mode,
    }

    #[state]
    pub enum Input {
        #[initial]
        Buffering,
    }

    #[state]
    pub enum Mode {
        #[initial]
        Fast,
        Slow,
    }

    #[handlers]
    impl Pipeline {
        #[on(Buffering)]
        fn on_buffering(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Data => defer!(),
                Ev::Flip => bubble!(),
            }
        }

        #[on(Fast)]
        fn on_fast(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Slow)
        }

        #[on(Slow)]
        fn on_slow(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Fast)
        }
    }
}

fn main() {}
