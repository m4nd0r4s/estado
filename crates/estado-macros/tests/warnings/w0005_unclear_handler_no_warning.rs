//! No W0005 when the macro cannot be sure: a catch-all arm that transitions,
//! a handler that does not `match` its event, an event expression that is a
//! function call, and an import-style single-segment event.
#![deny(deprecated)]

use std::time::Duration;

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Start,
    Timeout,
    Wake,
    Ping,
}

fn wake_up() -> Ev {
    Ev::Wake
}

#[machine(event = Ev)]
mod client {
    use super::*;
    use Ev::Ping;

    #[root]
    pub enum Client {
        #[initial]
        Idle,
        // The catch-all arm transitions, so it may handle the event.
        #[after(Duration::from_secs(1) => Ev::Timeout)]
        CatchAll,
        // The handler does not match its event at all.
        #[after(Duration::from_secs(1) => Ev::Timeout)]
        Opaque,
        // A function call is not a variant: not checked.
        #[after(Duration::from_secs(1) => wake_up())]
        Computed,
        // A single segment may be an import: not checked.
        #[after(Duration::from_secs(1) => Ping)]
        Imported,
    }

    #[handlers]
    impl Client {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(CatchAll),
                _ => bubble!(),
            }
        }

        #[on(CatchAll)]
        fn on_catch_all(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(Opaque),
                _ => goto!(Idle),
            }
        }

        #[on(Opaque)]
        fn on_opaque(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            if matches!(event, Ev::Timeout) {
                return goto!(Computed);
            }
            stay!()
        }

        #[on(Computed)]
        fn on_computed(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Imported)
        }

        #[on(Imported)]
        fn on_imported(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            goto!(Idle)
        }
    }
}

fn main() {}
