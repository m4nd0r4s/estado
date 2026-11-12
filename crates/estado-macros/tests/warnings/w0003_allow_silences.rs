//! `allow(W0003)` generates nothing, whatever the lint level.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002, W0003))]
mod uploader {
    use super::*;

    #[root]
    pub enum Uploader {
        #[initial]
        Idle,
        Stuck,
    }

    #[handlers]
    impl Uploader {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Stuck)
        }

        #[on(Stuck)]
        fn on_stuck(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            defer!()
        }
    }
}

fn main() {}
