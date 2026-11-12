//! `send` on an unstarted machine runs the whole start first, `#[always]`
//! and raised events included, and initial entries see the configuration
//! being entered (spec 4.5, ADR 0013 items 6 and 7).
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Effect, Machine, Next, Overflow, SendError, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Note(u8),
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, boot::Boot, S>, line: String) {
    cx.ctx_mut().push(line);
}

#[machine(event = Event, ctx = Vec<String>, raise_cap = 4, allow(W0002))]
mod boot {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Boot {
        #[initial]
        Booting,
        Ready,
    }

    #[handlers]
    impl Boot {
        #[entry(Booting)]
        fn entry_booting(cx: &mut Cx<Self>) {
            note(cx, "entry Booting".to_owned());
            cx.raise(Event::Note(1));
        }

        #[always(Booting)]
        fn always_booting(cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, "always Booting".to_owned());
            goto!(Ready)
        }

        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Ready handles {event:?}"));
            stay!()
        }
    }
}

#[test]
fn implicit_start_runs_always_and_raised_events_before_the_event() {
    let mut implicit = Machine::<boot::Boot>::new(Vec::new());
    implicit.send(Event::Go).unwrap();

    let mut explicit = Machine::<boot::Boot>::new(Vec::new());
    explicit.start().unwrap();
    explicit.send(Event::Go).unwrap();

    let expected = [
        "entry Booting",
        "always Booting",
        "Ready handles Note(1)",
        "Ready handles Go",
    ];
    assert_eq!(*implicit.ctx(), expected);
    assert_eq!(*explicit.ctx(), expected);
}

#[machine(event = Event, ctx = Vec<bool>, allow(W0001, W0002))]
mod panel {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Panel {
        #[initial]
        Running(Running),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        #[region]
        pub light: Light,
        #[region]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Off,
        On,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Idle,
        Busy,
    }

    #[handlers]
    impl Panel {
        /// Entered second (regions enter in declaration order); asks about
        /// the sibling region entered first.
        #[entry(Idle)]
        fn entry_idle(cx: &mut Cx<Self>) {
            let light_is_off = cx.in_state(Panel::OFF);
            cx.ctx_mut().push(light_is_off);
        }
    }
}

#[test]
fn initial_entries_see_the_configuration_being_entered() {
    let mut machine = Machine::<panel::Panel>::new(Vec::new());
    machine.start().unwrap();
    assert_eq!(*machine.ctx(), [true]);
}

/// Accepts one effect, then overflows.
struct OneEffect(usize);

impl<Command> Sink<Command> for OneEffect {
    fn push(&mut self, _: Effect<Command>) -> Result<(), Overflow> {
        self.0 += 1;
        if self.0 > 1 { Err(Overflow) } else { Ok(()) }
    }
}

#[derive(Debug)]
pub enum Beep {
    Short,
}

#[machine(event = Event, effect = Beep, allow(W0002))]
mod noisy {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Noisy {
        #[initial]
        Loud,
    }

    #[handlers]
    impl Noisy {
        #[entry(Loud)]
        fn entry_loud(cx: &mut Cx<Self>) {
            cx.emit(Beep::Short);
            cx.emit(Beep::Short);
        }
    }
}

#[test]
fn a_fault_during_the_implicit_start_poisons_the_machine() {
    let mut machine = Machine::<noisy::Noisy>::new(());
    let error = machine.send_into(Event::Go, &mut OneEffect(0)).unwrap_err();
    assert!(matches!(error, SendError::EffectOverflow));
    assert!(machine.is_poisoned());
    assert!(matches!(
        machine.send(Event::Go).unwrap_err(),
        SendError::Poisoned
    ));
}

#[test]
fn a_fault_during_an_explicit_start_poisons_the_machine() {
    let mut machine = Machine::<noisy::Noisy>::new(());
    assert!(matches!(
        machine.start_into(&mut OneEffect(0)).unwrap_err(),
        SendError::EffectOverflow
    ));
    assert!(machine.is_poisoned());
}

#[machine(event = Event, ctx = Vec<String>, raise_cap = 4, max_microsteps = 2, allow(W0002))]
mod tight {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Tight {
        #[initial]
        Booting,
        Ready,
    }

    #[handlers]
    impl Tight {
        #[entry(Booting)]
        fn entry_booting(cx: &mut Cx<Self>) {
            cx.raise(Event::Note(1));
        }

        #[always(Booting)]
        fn always_booting() -> Next<Self> {
            goto!(Ready)
        }

        #[on(Ready)]
        fn on_ready() -> Next<Self> {
            stay!()
        }
    }
}

#[test]
fn the_implicit_start_and_the_event_share_one_livelock_budget() {
    // Start: always + raised Note = 2 microsteps; the event is the third.
    let mut explicit = Machine::<tight::Tight>::new(Vec::new());
    explicit.start().unwrap();
    assert!(explicit.send(Event::Go).is_ok());

    let mut implicit = Machine::<tight::Tight>::new(Vec::new());
    let error = implicit.send(Event::Go).unwrap_err();
    assert!(
        matches!(error, SendError::Livelock { microsteps: 3, .. }),
        "{error:?}"
    );
    assert!(implicit.is_poisoned());
}
