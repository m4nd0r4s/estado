//! No W0005 when the event is matched by the state, by an ancestor, by a
//! substate, or when the state has no timer variant to look for.
#![deny(deprecated)]

use std::time::Duration;

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Start,
    Timeout,
    Expired(u32),
    Tick,
}

#[machine(event = Ev)]
mod client {
    use super::*;

    #[root]
    pub enum Client {
        #[initial]
        Idle,
        // Matched by the state itself.
        #[after(Duration::from_secs(3) => Ev::Timeout)]
        Connecting,
        Session(Session),
    }

    // Matched by the compound state's own handler, built by a closure.
    #[state]
    #[after(|session: &Session| Duration::from_secs(u64::from(session.seconds)) => |session: &Session| Ev::Expired(session.seconds))]
    pub struct Session {
        pub seconds: u32,
        #[substates]
        pub phase: Phase,
    }

    #[state]
    pub enum Phase {
        #[initial]
        // Matched by an ancestor.
        #[after(Duration::from_secs(1) => Ev::Expired(0))]
        Active,
        // Matched by this state's `|` pattern next to another variant.
        #[after(Duration::from_secs(1) => Ev::Tick)]
        Paused,
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

        #[on(Connecting)]
        fn on_connecting(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Timeout => goto!(Session { seconds: 5 }),
                _ => bubble!(),
            }
        }

        #[on(Session)]
        fn on_session(_up: SessionFields<'_>, _cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Expired(_) => goto!(Idle),
                _ => bubble!(),
            }
        }

        #[on(Active)]
        fn on_active(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(Paused),
                _ => bubble!(),
            }
        }

        #[on(Paused)]
        fn on_paused(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Tick | Ev::Start => goto!(Active),
                _ => bubble!(),
            }
        }
    }
}

fn main() {}
