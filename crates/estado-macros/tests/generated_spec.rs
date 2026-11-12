//! What the options and the layout of a generated machine promise (spec 3.5,
//! 6.1, AC-15), checked on the `Spec` impl the macro writes: the options
//! reach the associated constants and types, a machine that needs no runtime
//! costs nothing over its parts, and a panic in a handler poisons it.
#![allow(dead_code, missing_docs)]

use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use estado_core::{
    DeferOverflowPolicy, InlineEffects, Machine, Next, SendError, Slot, Spec, UnhandledPolicy,
};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beep;

#[machine(
    event = Event,
    effect = Beep,
    ctx = u8,
    unhandled = error,
    defer_cap = 3,
    defer_overflow = drop_oldest,
    raise_cap = 5,
    fx_inline = 2,
    max_microsteps = 11,
    allow(W0002)
)]
mod configured {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Configured {
        #[initial]
        Idle,
        Busy,
    }

    #[handlers]
    impl Configured {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Busy),
                Event::Crash => panic!("a handler failed"),
            }
        }
    }
}

use configured::Configured;

#[test]
fn the_options_reach_the_spec_impl() {
    assert_eq!(Configured::UNHANDLED, UnhandledPolicy::Error);
    assert_eq!(Configured::DEFER_OVERFLOW, DeferOverflowPolicy::DropOldest);
    assert_eq!(Configured::MAX_MICROSTEPS, 11);
    assert_eq!(Configured::NAME, "Configured");
    // `fx_inline = 2` is the capacity of the effect buffer.
    let _: <Configured as Spec>::Effects = InlineEffects::<Beep, 2>::default();
    // `raise_cap` and `defer_cap` size the queues, which this machine
    // generates because it names the caps.
    assert!(size_of::<<Configured as Spec>::Runtime>() > 0);
}

#[test]
fn a_panic_in_a_handler_poisons_the_machine() {
    let mut machine = Machine::<Configured>::new(0);

    let outcome = catch_unwind(AssertUnwindSafe(|| machine.send(Event::Crash)));

    assert!(outcome.is_err(), "the panic propagates");
    assert!(machine.is_poisoned());
    assert!(matches!(machine.send(Event::Go), Err(SendError::Poisoned)));
}

#[machine(event = Event, ctx = u8, allow(W0002))]
mod plain {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Plain {
        #[initial]
        First,
        Second,
        Third(u8),
    }

    #[handlers]
    impl Plain {
        #[on(First)]
        fn first(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Second),
                Event::Crash => goto!(Third(1)),
            }
        }
    }
}

use plain::Plain;

/// AC-15 with errata 1: a machine without optional features adds nothing to
/// its root and its context, and the slot hides its markers in the niches of
/// the root.
#[test]
fn ac_15_a_generated_machine_costs_nothing_over_its_parts() {
    assert_eq!(
        size_of::<<Plain as Spec>::Runtime>(),
        0,
        "no queues, epochs or flags: `NoRuntime`"
    );
    assert_eq!(size_of::<Slot<Plain>>(), size_of::<Plain>());
    assert_eq!(size_of::<Machine<Plain>>(), size_of::<(Plain, u8)>());
}
