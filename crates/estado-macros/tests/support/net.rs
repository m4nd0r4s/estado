//! The `Net` machine of the timer and invoke tests, and the helpers that
//! drive it and collect the effects.
#![allow(dead_code, missing_docs, unused_imports)]

use std::time::Duration;

use estado_core::{Cx, Effect, FnSink, IgnoreReason, InvokeKey, Machine, Next, Spec, TimerKey};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connect,
    Connected(u32),
    Timeout,
    Retry,
    Hang,
    Add(u32),
    Expired(u32),
    Pause,
    Resume,
    SlowTick,
    FastTick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Dial(u8),
    Probe(u32),
}

pub type Fx = Effect<Command>;

// W0005 is allowed on purpose: the timers of `Ticking` (`SlowTick`) and of
// `Session` (`Expired`) fire events no handler matches, so the tests can
// observe an unhandled timer event.
#[machine(event = Event, effect = Command, allow(W0005))]
pub mod net {
    use super::*;

    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Net {
        #[initial]
        Idle,
        /// A timer and an invoke of one state instance share an epoch.
        #[after(Duration::from_secs(3) => Event::Timeout)]
        #[invoke(|connecting| Command::Dial(*connecting.attempt))]
        Connecting {
            attempt: u8,
        },
        /// The duration is computed from the data of the state on entry.
        #[after(|backing_off| Duration::from_millis(100 << *backing_off.attempt) => Event::Retry)]
        Backoff {
            attempt: u8,
        },
        Online(Online),
        /// Two timers: the index is part of the key.
        #[after(Duration::from_secs(1) => Event::SlowTick)]
        #[after(Duration::from_millis(250) => Event::FastTick)]
        Ticking,
        Session(Session),
    }

    /// The event of a timer is built when it fires, from the current data.
    #[state]
    #[after(Duration::from_secs(10) => |online: &Online| Event::Expired(online.count))]
    #[invoke(|online: &Online| Command::Probe(online.count))]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Online {
        pub count: u32,
    }

    /// A compound state with a timer of its own and a substate with another.
    #[state]
    #[after(|session: &Session| Duration::from_secs(u64::from(session.seconds)) => |session: &Session| Event::Expired(session.seconds))]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Session {
        pub seconds: u32,
        #[substates]
        pub phase: Phase,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Phase {
        #[initial]
        Active,
        #[after(Duration::from_secs(60) => Event::Resume)]
        Paused,
    }

    #[handlers]
    impl Net {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Connect => goto!(Connecting { attempt: 0 }),
                Event::SlowTick => goto!(Ticking),
                Event::Hang => goto!(Session { seconds: 30 }),
                _ => bubble!(),
            }
        }

        #[on(Connecting)]
        fn on_connecting(
            connecting: ConnectingMut<'_>,
            _cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            match event {
                Event::Timeout => goto!(Backoff {
                    attempt: *connecting.attempt + 1
                }),
                Event::Connected(count) => goto!(Online(Online { count: *count })),
                Event::Hang => goto!(Idle),
                _ => bubble!(),
            }
        }

        #[on(Backoff)]
        fn on_backoff(backoff: BackoffMut<'_>, _cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Retry => goto!(Connecting {
                    attempt: *backoff.attempt
                }),
                _ => bubble!(),
            }
        }

        #[on(Online)]
        fn on_online(online: &mut Online, _cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Add(amount) => {
                    online.count += amount;
                    stay!()
                }
                Event::Expired(count) => {
                    online.count = *count + 1000;
                    stay!()
                }
                Event::Hang => goto!(Idle),
                _ => bubble!(),
            }
        }

        #[on(Ticking)]
        fn on_ticking(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::FastTick => goto!(Idle),
                Event::Hang => goto!(Idle),
                _ => bubble!(),
            }
        }

        #[on(Active)]
        fn on_active(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Pause => goto!(Paused),
                _ => bubble!(),
            }
        }

        #[on(Paused)]
        fn on_paused(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Resume => goto!(Active),
                _ => bubble!(),
            }
        }

        #[on(Session)]
        fn on_session(_up: SessionFields<'_>, _cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Hang => goto!(Idle),
                _ => bubble!(),
            }
        }
    }
}

pub use net::{Net, Online};

pub fn started() -> Machine<Net> {
    let mut machine = Machine::<Net>::new(());
    machine.start().unwrap();
    machine
}

pub fn send(machine: &mut Machine<Net>, event: Event) -> Vec<Fx> {
    let mut effects = Vec::new();
    machine
        .send_into(event, &mut FnSink(|effect| effects.push(effect)))
        .unwrap();
    effects
}

pub fn fire(machine: &mut Machine<Net>, key: TimerKey) -> (Vec<Fx>, Option<IgnoreReason>) {
    let mut effects = Vec::new();
    let report = machine
        .fire_into(key, &mut FnSink(|effect| effects.push(effect)))
        .unwrap();
    (effects, report.ignored)
}

pub fn complete(
    machine: &mut Machine<Net>,
    key: InvokeKey,
    event: Event,
) -> (Vec<Fx>, Option<IgnoreReason>) {
    let mut effects = Vec::new();
    let report = machine
        .complete_into(key, event, &mut FnSink(|effect| effects.push(effect)))
        .unwrap();
    (effects, report.ignored)
}

pub fn timer(state: estado_core::StateId, epoch: u32, index: u8) -> TimerKey {
    TimerKey {
        state,
        epoch,
        index,
        child: estado_core::ChildPath::ROOT,
    }
}

pub fn invoke(state: estado_core::StateId, epoch: u32, index: u8) -> InvokeKey {
    InvokeKey {
        state,
        epoch,
        index,
        child: estado_core::ChildPath::ROOT,
    }
}
