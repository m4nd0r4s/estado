//! `#[always]` on machines written with `#[machine]` (spec 5.9, AC-03,
//! AC-07): the evaluation is the next microstep after the entry, before
//! internal events, and counts toward the livelock limit.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, SendError, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Note(u8),
    Poke,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, probe::Probe, S>, line: String) {
    cx.ctx_mut().push(line);
}

#[machine(event = Event, ctx = Vec<String>, allow(W0002))]
mod probe {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Probe {
        #[initial]
        Start,
        /// Entering it raises an event and schedules `always`.
        Checking {
            tries: u8,
        },
        Settled,
        Guarded(Guarded),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Guarded {
        pub level: u8,
        #[substates]
        pub inner: Inner,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Inner {
        #[initial]
        Calm,
        Tense,
    }

    #[handlers]
    impl Probe {
        #[entry(Checking)]
        fn entry_checking(checking: CheckingMut<'_>, cx: &mut Cx<Self>) {
            note(cx, format!("entry Checking ({})", checking.tries));
            cx.raise(Event::Note(9));
        }

        #[entry(Settled)]
        fn entry_settled(cx: &mut Cx<Self>) {
            note(cx, "entry Settled".to_owned());
        }

        #[on(Start)]
        fn on_start(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Start handles {event:?}"));
            match event {
                Event::Go => goto!(Checking { tries: 0 }),
                Event::Poke => goto!(Guarded { level: 1 }),
                _ => bubble!(),
            }
        }

        /// Evaluated as its own microstep, before the raised `Note(9)`.
        #[always(Checking)]
        fn always_checking(checking: CheckingMut<'_>, cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, format!("always Checking ({})", checking.tries));
            if *checking.tries >= 2 {
                goto!(Settled)
            } else {
                goto!(Checking {
                    tries: *checking.tries + 1
                })
            }
        }

        #[on(Settled)]
        fn on_settled(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Settled handles {event:?}"));
            match event {
                Event::Poke => goto!(Guarded { level: 3 }),
                _ => bubble!(),
            }
        }

        /// `stay!()` ends the evaluation; it runs once per entry, not on
        /// every later event.
        #[always(Calm)]
        fn always_calm(up: GuardedFields<'_>, cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, format!("always Calm (level {})", up.level));
            if *up.level > 2 { goto!(Tense) } else { stay!() }
        }

        #[on(Calm)]
        fn on_calm(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Calm handles {event:?}"));
            bubble!()
        }

        #[on(Tense)]
        fn on_tense(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Tense handles {event:?}"));
            bubble!()
        }
    }
}

pub use probe::Probe;

fn lines(machine: &mut Machine<Probe>) -> Vec<String> {
    std::mem::take(machine.ctx_mut())
}

fn started() -> Machine<Probe> {
    let mut machine = Machine::<Probe>::new(Vec::new());
    machine.start().unwrap();
    lines(&mut machine);
    machine
}

#[test]
fn always_runs_right_after_the_entry_and_before_internal_events() {
    let mut machine = started();

    let report = machine.send(Event::Go).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Start handles Go",
            "entry Checking (0)",
            // The always of `Checking` first, before the raised `Note(9)`.
            "always Checking (0)",
            "entry Checking (1)",
            "always Checking (1)",
            "entry Checking (2)",
            "always Checking (2)",
            "entry Settled",
            // The three raised notes are only now delivered; `Settled` does
            // not handle them.
            "Settled handles Note(9)",
            "Settled handles Note(9)",
            "Settled handles Note(9)",
        ]
    );
    // The event, three always evaluations and three internal events.
    assert_eq!(report.microsteps, 7);
    assert_eq!(report.transitions, 4);
    assert_eq!(machine.path().to_string(), "Settled");
}

#[test]
fn a_state_with_always_that_stays_runs_it_once_per_entry() {
    let mut machine = started();

    machine.send(Event::Poke).unwrap();

    assert_eq!(
        lines(&mut machine),
        ["Start handles Poke", "always Calm (level 1)"]
    );
    assert_eq!(machine.path().to_string(), "Guarded/Calm");

    // A later event does not evaluate it again.
    machine.send(Event::Note(1)).unwrap();
    assert_eq!(lines(&mut machine), ["Calm handles Note(1)"]);
}

#[test]
fn always_of_a_substate_receives_the_fields_of_its_compound_and_may_transition() {
    let mut machine = started();
    machine.send(Event::Go).unwrap();
    lines(&mut machine);

    machine.send(Event::Poke).unwrap();

    assert_eq!(
        lines(&mut machine),
        ["Settled handles Poke", "always Calm (level 3)"]
    );
    assert_eq!(machine.path().to_string(), "Guarded/Tense");
}

#[machine(event = Event, max_microsteps = 6)]
mod spin {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Spin {
        #[initial]
        Idle,
        Ping,
        Pong,
    }

    #[handlers]
    impl Spin {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Ping),
                _ => bubble!(),
            }
        }

        #[always(Ping)]
        fn always_ping(_cx: &mut Cx<Self>) -> Next<Self> {
            goto!(Pong)
        }

        #[always(Pong)]
        fn always_pong(_cx: &mut Cx<Self>) -> Next<Self> {
            goto!(Ping)
        }
    }
}

#[test]
fn a_cycle_of_always_transitions_poisons_the_machine_after_max_microsteps() {
    let mut machine = Machine::<spin::Spin>::new(());

    let error = machine.send(Event::Go).unwrap_err();

    assert!(matches!(error, SendError::Livelock { microsteps, .. } if microsteps > 6));
    assert!(machine.is_poisoned());
}

#[machine(event = Event, ctx = Vec<String>, allow(W0002))]
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
        #[always(Booting)]
        fn always_booting(cx: &mut Cx<Self>) -> Next<Self> {
            cx.ctx_mut().push("always Booting".to_owned());
            goto!(Ready)
        }
    }
}

#[test]
fn the_initial_state_may_have_an_always() {
    let mut machine = Machine::<boot::Boot>::new(Vec::new());

    let report = machine.start().unwrap().report;

    assert_eq!(machine.ctx(), &["always Booting"]);
    assert_eq!(machine.path().to_string(), "Ready");
    assert_eq!(report.microsteps, 1);
}
