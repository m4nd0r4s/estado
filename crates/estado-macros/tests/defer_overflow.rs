//! A full defer queue under `defer_overflow = error` (ADR 0014, amendment):
//! the external event that overflows is refused before anything ran and the
//! machine stays usable; a raised event that overflows cuts the step
//! half-way and poisons the machine.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, SendError, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Hold,
    Ripple,
    Flood,
    Ping,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, shelf::Shelf, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>, defer_cap = 1, allow(W0002, W0003))]
mod shelf {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Shelf {
        #[initial]
        Open,
        Closed,
    }

    #[handlers]
    impl Shelf {
        #[entry(Closed)]
        fn entry_closed(cx: &mut Cx<Self>) {
            note(cx, "entry Closed");
        }

        #[on(Open)]
        fn on_open(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Hold => defer!(),
                // Raises a deferrable event and stays.
                Event::Ripple => {
                    note(cx, "Open handles Ripple");
                    cx.raise(Event::Hold);
                    stay!()
                }
                // Raises two deferrable events and transitions.
                Event::Flood => {
                    note(cx, "Open handles Flood");
                    cx.raise(Event::Hold);
                    cx.raise(Event::Hold);
                    goto!(Closed)
                }
                Event::Ping => {
                    note(cx, "Open handles Ping");
                    stay!()
                }
            }
        }

        #[on(Closed)]
        fn on_closed(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Hold => defer!(),
                _ => bubble!(),
            }
        }
    }
}

use shelf::Shelf;

fn started() -> Machine<Shelf> {
    let mut machine = Machine::<Shelf>::new(Vec::new());
    machine.start().unwrap();
    machine
}

fn assert_poisoned(machine: &mut Machine<Shelf>) {
    assert!(machine.is_poisoned());
    assert!(matches!(
        machine.send(Event::Ping).unwrap_err(),
        SendError::Poisoned
    ));
    assert!(machine.try_state().is_none());
}

#[test]
fn an_external_event_that_overflows_is_given_back_and_the_machine_stays_usable() {
    let mut machine = started();
    machine.send(Event::Hold).unwrap();

    let error = machine.send(Event::Hold).unwrap_err();

    assert_eq!(error, SendError::DeferOverflow(Event::Hold));
    assert!(!machine.is_poisoned());
    assert_eq!(*machine.state(), Shelf::Open);
    let report = machine.send(Event::Ping).unwrap().report;
    assert!(report.handled);
}

#[test]
fn a_raised_event_that_overflows_after_a_handler_ran_poisons_the_machine() {
    let mut machine = started();
    machine.send(Event::Hold).unwrap();

    let error = machine.send(Event::Ripple).unwrap_err();

    assert_eq!(error, SendError::DeferOverflow(Event::Hold));
    assert_eq!(machine.ctx().as_slice(), ["Open handles Ripple"]);
    assert_poisoned(&mut machine);
}

#[test]
fn a_raised_event_that_overflows_after_a_transition_poisons_the_machine() {
    let mut machine = started();

    let error = machine.send(Event::Flood).unwrap_err();

    // The first raised `Hold` fits, the second does not.
    assert_eq!(error, SendError::DeferOverflow(Event::Hold));
    assert_eq!(
        machine.ctx().as_slice(),
        ["Open handles Flood", "entry Closed"]
    );
    assert_poisoned(&mut machine);
}
