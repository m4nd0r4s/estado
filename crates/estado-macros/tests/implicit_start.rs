//! `send` on an unstarted machine runs the whole start first, `#[always]`
//! and raised events included, and initial entries see the configuration
//! being entered (spec 4.5, ADR 0013 items 6 and 7).
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, Sink};
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
