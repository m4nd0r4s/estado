//! A panic in the event expression of an `#[after(.. => event)]` timer, which
//! runs when the timer fires, poisons the machine like a panic in a handler
//! (spec 5.11).
#![allow(dead_code, missing_docs)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use estado_core::{Cx, Effect, FnSink, Machine, Next, SendError, TimerKey};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Arm,
    Boom,
}

fn explode() -> Event {
    panic!("the timer event cannot be built");
}

#[machine(event = Event, allow(W0002, W0005))]
mod fuse {
    use super::*;

    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Fuse {
        #[initial]
        Idle,
        #[after(Duration::from_secs(1) => explode())]
        Armed,
    }

    #[handlers]
    impl Fuse {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Arm => goto!(Armed),
                _ => bubble!(),
            }
        }
    }
}

use fuse::Fuse;

fn armed() -> (Machine<Fuse>, TimerKey) {
    let mut machine = Machine::<Fuse>::new(());
    let mut effects = Vec::new();
    machine
        .send_into(Event::Arm, &mut FnSink(|effect| effects.push(effect)))
        .unwrap();
    let key = effects
        .into_iter()
        .find_map(|effect| match effect {
            Effect::StartTimer { key, .. } => Some(key),
            _ => None,
        })
        .expect("entering `Armed` starts its timer");
    (machine, key)
}

#[test]
fn a_panic_building_the_timer_event_poisons_the_machine() {
    let (mut machine, key) = armed();

    let outcome = catch_unwind(AssertUnwindSafe(|| machine.fire(key)));

    assert!(outcome.is_err(), "the panic propagates");
    assert!(machine.is_poisoned());
    assert!(matches!(
        machine.send(Event::Boom),
        Err(SendError::Poisoned)
    ));
    assert!(matches!(machine.fire(key), Err(SendError::Poisoned)));
}

#[test]
fn a_stale_timer_does_not_run_the_event_expression() {
    let (mut machine, key) = armed();
    let stale = TimerKey {
        epoch: key.epoch + 1,
        ..key
    };

    let step = machine.fire(stale).unwrap();

    assert!(step.report.ignored.is_some());
    assert!(!machine.is_poisoned());
}
