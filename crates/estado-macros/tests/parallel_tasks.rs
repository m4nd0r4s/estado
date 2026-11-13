//! Timers and invokes on a parallel state (ADR 0012): the parallel state's
//! own tasks against the tasks of its regions (scheduling parent first,
//! regions in declaration order; cancelling the other way round), stale keys
//! after the parallel state is entered again, and a constructor edge inside a
//! region whose states have tasks.
#![allow(dead_code, missing_docs)]

use std::time::Duration;

use estado_core::{ChildPath, Effect, IgnoreReason, InvokeKey, Machine, Next, TimerKey};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Join,
    Leave,
    Connected,
    Tick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Dial,
    Heartbeat,
    Listen,
}

const SECOND: Duration = Duration::from_secs(1);

// W0005 is allowed on purpose: no handler matches `Event::Tick`, which only
// the timer of the state fires; the tests look at the effects it produces.
#[machine(event = Event, effect = Command, allow(W0002, W0005))]
mod session {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Session {
        #[initial]
        Idle,
        Online(Online),
    }

    #[state]
    #[after(SECOND => Event::Tick)]
    #[invoke(Command::Heartbeat)]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Online {
        pub id: u8,
        #[region]
        pub link: Link,
        #[region]
        pub audio: Audio,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Link {
        #[initial]
        #[after(SECOND => Event::Tick)]
        #[invoke(Command::Dial)]
        Connecting,
        #[after(SECOND => Event::Tick)]
        Up,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Audio {
        #[initial]
        #[after(SECOND => Event::Tick)]
        #[invoke(Command::Listen)]
        Muted,
    }

    #[handlers]
    impl Session {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Join => goto!(Online { id: 1 }),
                _ => bubble!(),
            }
        }

        #[on(Connecting)]
        fn connecting(event: &Event) -> Next<Self> {
            match event {
                Event::Connected => goto!(Up, |_old: Exiting![Connecting]| Link::Up),
                _ => bubble!(),
            }
        }

        #[on(Online)]
        fn online(event: &Event) -> Next<Self> {
            match event {
                Event::Leave => goto!(Idle),
                _ => bubble!(),
            }
        }
    }
}

use session::Session;

fn timer(state: estado_core::StateId, epoch: u32) -> TimerKey {
    TimerKey {
        state,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

fn invoke(state: estado_core::StateId, epoch: u32) -> InvokeKey {
    InvokeKey {
        state,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

fn send(machine: &mut Machine<Session>, event: Event) -> Vec<Effect<Command>> {
    let mut effects = Vec::new();
    machine
        .send_with(event, |effect| effects.push(effect))
        .unwrap();
    effects
}

#[test]
fn the_parallel_state_schedules_before_its_regions_and_cancels_after_them() {
    let mut machine = Machine::<Session>::new(());

    let entered = send(&mut machine, Event::Join);

    assert_eq!(
        entered,
        [
            Effect::StartTimer {
                key: timer(Session::ONLINE_ID, 1),
                after: SECOND
            },
            Effect::Invoke {
                key: invoke(Session::ONLINE_ID, 1),
                task: Command::Heartbeat
            },
            Effect::StartTimer {
                key: timer(Session::CONNECTING_ID, 1),
                after: SECOND
            },
            Effect::Invoke {
                key: invoke(Session::CONNECTING_ID, 1),
                task: Command::Dial
            },
            Effect::StartTimer {
                key: timer(Session::MUTED_ID, 1),
                after: SECOND
            },
            Effect::Invoke {
                key: invoke(Session::MUTED_ID, 1),
                task: Command::Listen
            },
        ]
    );

    let left = send(&mut machine, Event::Leave);

    // Regions in reverse declaration order, deepest first, then the parallel
    // state itself; the timers of a state before its invokes.
    assert_eq!(
        left,
        [
            Effect::CancelTimer(timer(Session::MUTED_ID, 1)),
            Effect::Cancel(invoke(Session::MUTED_ID, 1)),
            Effect::CancelTimer(timer(Session::CONNECTING_ID, 1)),
            Effect::Cancel(invoke(Session::CONNECTING_ID, 1)),
            Effect::CancelTimer(timer(Session::ONLINE_ID, 1)),
            Effect::Cancel(invoke(Session::ONLINE_ID, 1)),
        ]
    );
}

#[test]
fn keys_of_a_previous_visit_to_the_parallel_state_are_stale() {
    let mut machine = Machine::<Session>::new(());
    send(&mut machine, Event::Join);
    send(&mut machine, Event::Leave);
    send(&mut machine, Event::Join);

    // Second visit: every state with tasks is at epoch 2.
    let stale = machine.fire(timer(Session::ONLINE_ID, 1)).unwrap().report;
    assert_eq!(stale.ignored, Some(IgnoreReason::Stale));
    let stale_region = machine
        .fire(timer(Session::CONNECTING_ID, 1))
        .unwrap()
        .report;
    assert_eq!(stale_region.ignored, Some(IgnoreReason::Stale));

    let current = machine
        .fire(timer(Session::CONNECTING_ID, 2))
        .unwrap()
        .report;
    assert_ne!(current.ignored, Some(IgnoreReason::Stale));
    let current_parallel = machine.fire(timer(Session::ONLINE_ID, 2)).unwrap().report;
    assert_ne!(current_parallel.ignored, Some(IgnoreReason::Stale));
}

#[test]
fn a_constructor_edge_inside_a_region_cancels_and_starts_only_its_own_tasks() {
    let mut machine = Machine::<Session>::new(());
    send(&mut machine, Event::Join);

    let effects = send(&mut machine, Event::Connected);

    assert_eq!(
        effects,
        [
            Effect::CancelTimer(timer(Session::CONNECTING_ID, 1)),
            Effect::Cancel(invoke(Session::CONNECTING_ID, 1)),
            Effect::StartTimer {
                key: timer(Session::UP_ID, 1),
                after: SECOND
            },
        ]
    );
    // The parallel state and the other region were not touched.
    let untouched = machine.fire(timer(Session::MUTED_ID, 1)).unwrap().report;
    assert_ne!(untouched.ignored, Some(IgnoreReason::Stale));
}
