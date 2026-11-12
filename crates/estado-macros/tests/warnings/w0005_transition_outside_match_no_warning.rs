//! No W0005 when the handler can transition outside its `match` (before or
//! after it), since that reacts to every event, or when an arm pattern is an
//! all-uppercase name that may be a constant or a renamed import. Guards the
//! false positives found by the fourth review.
#![deny(deprecated)]

use std::time::Duration;

use estado_core::{Cx, Next};
use estado_macros::machine;

pub enum Ev {
    Start,
    Timeout,
    Other,
}

const STOP: bool = true;

#[machine(event = Ev, ctx = bool)]
mod guard {
    use super::*;
    use Ev::Timeout as T;

    #[root]
    pub enum Guard {
        #[initial]
        Idle,
        // An early return before the match fires for `Timeout` too.
        #[after(Duration::from_secs(1) => Ev::Timeout)]
        Before,
        // A check after the match fires for `Timeout` too.
        #[after(Duration::from_secs(1) => Ev::Timeout)]
        After,
        // `T` is `Ev::Timeout` under another name.
        #[after(Duration::from_secs(1) => Ev::Timeout)]
        Renamed,
    }

    #[handlers]
    impl Guard {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Start => goto!(Before),
                Ev::Other => goto!(After),
                _ => goto!(Renamed),
            }
        }

        #[on(Before)]
        fn on_before(cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            if *cx.ctx() {
                return goto!(Idle);
            }
            match event {
                Ev::Start => stay!(),
                _ => bubble!(),
            }
        }

        #[on(After)]
        fn on_after(cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            let next = match event {
                Ev::Start => stay!(),
                _ => bubble!(),
            };
            if *cx.ctx() && STOP {
                return goto!(Idle);
            }
            next
        }

        #[on(Renamed)]
        fn on_renamed(_cx: &mut Cx<Self>, event: &Ev) -> Next<Self> {
            match event {
                T => goto!(Idle),
                _ => bubble!(),
            }
        }
    }
}

fn main() {}
