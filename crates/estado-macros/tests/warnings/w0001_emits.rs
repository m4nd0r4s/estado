//! W0001 is reported, once, at the topmost state no edge leads to. A state
//! reached through a region, a substate or an `#[always]` is reachable.
#![deny(deprecated)]

use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        Orphan(Orphan),
    }

    #[state]
    pub struct Active {
        #[substates]
        sub: ActiveSub,
    }

    #[state]
    pub enum ActiveSub {
        #[initial]
        Playing,
        Paused,
    }

    // Nothing has `goto!(Orphan)`: reported here, not at its substates.
    #[state]
    pub struct Orphan {
        #[substates]
        sub: OrphanSub,
    }

    #[state]
    pub enum OrphanSub {
        #[initial]
        Lost,
        Found,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Active)
        }

        #[on(Playing)]
        fn on_playing(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Paused)
        }
    }
}

fn main() {}
