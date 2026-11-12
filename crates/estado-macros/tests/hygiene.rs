//! ADR 0013 item 15: the expressions of `#[after]` and `#[invoke]` are pasted
//! into generated functions that have locals named `epoch`, `children`,
//! `runtime` and `key`. A function of the user's with one of those names must
//! still be what the expression calls.
//!
//! Before the fix `let epoch = ..` shadowed the user's `epoch()` and the
//! generated code failed with E0618 ("expected function, found `u32`").
#![allow(dead_code, missing_docs)]

use std::time::Duration;

use estado_core::{Effect, Machine, Next};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Arm,
    Fired(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Probe(u64),
}

// W0005 is allowed on purpose: `Armed` has no handler, and the test relies on
// the event its timer builds being unhandled.
#[machine(event = Event, effect = Command, allow(W0002, W0005))]
mod clock {
    use super::*;

    fn epoch() -> u64 {
        1
    }

    fn key() -> u64 {
        2
    }

    fn runtime() -> u64 {
        4
    }

    fn children() -> u64 {
        8
    }

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Clock {
        #[initial]
        Idle,
        #[after(Duration::from_millis(epoch() + key() + runtime() + children()) => Event::Fired(key()))]
        #[invoke(Command::Probe(epoch() + children()))]
        Armed,
    }

    #[handlers]
    impl Clock {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Arm => goto!(Armed),
                _ => bubble!(),
            }
        }
    }
}

use clock::Clock;

#[test]
fn user_functions_named_like_generated_locals_are_still_called() {
    let mut machine = Machine::<Clock>::new(());
    let mut effects = Vec::new();
    machine
        .send_with(Event::Arm, |effect| effects.push(effect))
        .unwrap();

    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::StartTimer { after, .. } if *after == Duration::from_millis(15)
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Invoke {
            task: Command::Probe(9),
            ..
        }
    )));

    // The event of the timer is built by the lookup, which has locals of the
    // same names.
    let key = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::StartTimer { key, .. } => Some(*key),
            _ => None,
        })
        .unwrap();
    let report = machine.fire(key).unwrap().report;
    // `Armed` has no handler: the event the timer built was unhandled, which
    // only happens when the lookup found the instance and called `key()`.
    assert!(!report.handled);
    assert_ne!(report.ignored, Some(estado_core::IgnoreReason::Stale));
}
