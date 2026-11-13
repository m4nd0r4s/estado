//! ADR 0015: `Cx::in_state` on a machine that does not track the active
//! configuration panics with a message naming `tracks_active`, instead of
//! answering `false` for ever.
//!
//! The macro turns tracking on when it sees `in_state` inside the machine
//! module. A call from a helper outside the module is invisible to it, which
//! is the case here (`forced_features.rs` has the same machine with
//! `tracks_active` written).
#![allow(dead_code, missing_docs)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use estado_core::{Cx, Machine, Next, Sink, Spec};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Probe,
}

/// Outside the machine module: the scan of the module does not see it.
fn is_on<S: Sink<()>>(cx: &Cx<'_, lamp::Lamp, S>) -> bool {
    Cx::in_state(cx, lamp::Lamp::ON_ID)
}

#[machine(event = Event, allow(W0002))]
mod lamp {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lamp {
        #[initial]
        On,
    }

    #[handlers]
    impl Lamp {
        #[on(On)]
        fn on(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Probe => {
                    assert!(is_on(cx));
                    stay!()
                }
            }
        }
    }
}

use lamp::Lamp;

#[test]
fn in_state_without_tracking_panics_and_names_the_option() {
    const { assert!(!<Lamp as Spec>::TRACKS_ACTIVE) };
    let mut machine = Machine::<Lamp>::new(());

    let outcome = catch_unwind(AssertUnwindSafe(|| machine.send(Event::Probe)));

    let payload = outcome.expect_err("an untracked in_state must not answer");
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .expect("a text payload");
    assert!(message.contains("tracks_active"), "message: {message}");
    assert!(machine.is_poisoned());
}
