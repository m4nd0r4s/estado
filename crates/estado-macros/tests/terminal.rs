//! AC-10 on a machine written with `#[machine]`: `#[terminal]`, `#[on_done]`,
//! `is_done` and `take_output` (spec 5.10, ADR 0005). The cases mirror
//! `estado-core/tests/regions_and_final.rs`, which runs them on a hand-written
//! machine.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, IgnoreReason, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Drain,
    Finish(u32),
    Abort,
    Reset,
    Poke,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Receipt {
    pub units: u32,
    pub attempts: u8,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, flow::Flow, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>)]
mod flow {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Flow {
        #[initial]
        Waiting,
        Working(Working),
        Draining(Draining),
        #[terminal]
        Closed(Receipt),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Working {
        pub attempts: u8,
        #[substates]
        pub step: Step,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Step {
        #[initial]
        Running,
        #[terminal]
        Complete(u32),
    }

    /// A compound state with a final substate and no `#[on_done]`.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Draining {
        pub pending: u8,
        #[substates]
        pub phase: Phase,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Phase {
        #[initial]
        Flushing,
        #[terminal]
        Flushed,
    }

    #[handlers]
    impl Flow {
        #[entry(Closed)]
        fn entry_closed(_receipt: &mut Receipt, cx: &mut Cx<Self>) {
            note(cx, "entry Closed");
        }

        #[entry(Complete)]
        fn entry_complete(_units: &mut u32, cx: &mut Cx<Self>) {
            note(cx, "entry Complete");
        }

        #[on(Waiting)]
        fn on_waiting(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Waiting handles");
            match event {
                Event::Start => goto!(Working { attempts: 1 }),
                Event::Drain => goto!(Draining { pending: 3 }),
                _ => bubble!(),
            }
        }

        #[on(Running)]
        fn on_running(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Running handles");
            match event {
                Event::Finish(units) => goto!(Complete(*units)),
                _ => bubble!(),
            }
        }

        /// The synthetic microstep: the compound state reacts to its final
        /// substate, with the payload of the final state.
        #[on_done(Working)]
        fn working_done(up: WorkingFields<'_>, units: &u32, cx: &mut Cx<Self>) -> Next<Self> {
            note(
                cx,
                &format!("on_done Working ({units} units, {} attempts)", up.attempts),
            );
            goto!(Closed(Receipt {
                units: *units,
                attempts: *up.attempts,
            }))
        }

        #[on(Draining)]
        fn on_draining(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Draining handles");
            match event {
                Event::Reset => goto!(Waiting),
                _ => bubble!(),
            }
        }

        #[on(Flushing)]
        fn on_flushing(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Flushing handles");
            match event {
                Event::Poke => goto!(Flushed),
                _ => bubble!(),
            }
        }
    }
}

pub use flow::{Draining, Flow, Phase, Step, Working};

fn started() -> Machine<Flow> {
    let mut machine = Machine::<Flow>::new(Vec::new());
    machine.start().unwrap();
    machine.ctx_mut().clear();
    machine
}

fn lines(machine: &mut Machine<Flow>) -> Vec<String> {
    std::mem::take(machine.ctx_mut())
}

#[test]
fn on_done_runs_when_a_substate_enters_a_final_and_receives_its_payload() {
    let mut machine = started();
    machine.send(Event::Start).unwrap();
    lines(&mut machine);

    let report = machine.send(Event::Finish(42)).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Running handles",
            "entry Complete",
            "on_done Working (42 units, 1 attempts)",
            "entry Closed",
        ]
    );
    assert!(machine.is_done());
    // One microstep for the event, one for the completion.
    assert_eq!(report.microsteps, 2);
    assert_eq!(report.transitions, 2);
    // The root `#[terminal]` payload leaves by value, once.
    assert_eq!(
        machine.take_output(),
        Some(Receipt {
            units: 42,
            attempts: 1
        })
    );
    assert_eq!(machine.take_output(), None);
    assert!(machine.is_done());
}

#[test]
fn a_compound_without_on_done_stays_in_its_final_substate() {
    let mut machine = started();
    machine.send(Event::Drain).unwrap();
    assert_eq!(machine.path().to_string(), "Draining/Flushing");
    lines(&mut machine);

    let report = machine.send(Event::Poke).unwrap().report;

    assert_eq!(lines(&mut machine), ["Flushing handles"]);
    assert_eq!(machine.path().to_string(), "Draining/Flushed");
    // The completion check ran as a microstep and found nothing to do.
    assert_eq!(report.microsteps, 2);
    assert_eq!(report.transitions, 1);
    assert!(!machine.is_done());

    // The final substate has no handler; the compound's own handler leaves.
    machine.send(Event::Reset).unwrap();
    assert_eq!(lines(&mut machine), ["Draining handles"]);
    assert_eq!(machine.path().to_string(), "Waiting");
}

#[test]
fn a_root_terminal_makes_the_machine_done_and_later_sends_are_ignored() {
    let mut machine = started();
    machine.send(Event::Start).unwrap();
    machine.send(Event::Finish(5)).unwrap();
    assert!(machine.is_done());
    assert_eq!(machine.path().to_string(), "Closed");
    lines(&mut machine);

    let report = machine.send(Event::Poke).unwrap().report;

    assert!(!report.handled);
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert_eq!(report.microsteps, 0);
    assert!(lines(&mut machine).is_empty());
    assert!(!machine.is_poisoned());
}

#[test]
fn a_machine_that_is_not_in_a_root_terminal_has_no_output() {
    let mut machine = started();
    assert!(!machine.is_done());
    assert_eq!(machine.take_output(), None);
}

#[test]
fn the_graph_marks_terminals_and_the_completion_edge() {
    use estado_core::{EdgeKind, Spec, StateKind};
    let graph = <Flow as Spec>::GRAPH;
    assert_eq!(graph.state(Flow::CLOSED).kind, StateKind::Final);
    assert_eq!(graph.state(Flow::COMPLETE).kind, StateKind::Final);
    assert_eq!(graph.state(Flow::COMPLETE).parent, Some(Flow::WORKING));
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.from == Flow::WORKING && edge.kind == EdgeKind::Done)
    );
}

#[machine(event = Event)]
mod unit_flow {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum UnitFlow {
        #[initial]
        Open,
        #[terminal]
        Shut,
    }

    #[handlers]
    impl UnitFlow {
        #[on(Open)]
        fn on_open(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Abort => goto!(Shut),
                _ => bubble!(),
            }
        }
    }
}

#[test]
fn a_terminal_without_payload_gives_the_unit_output() {
    let mut machine = Machine::<unit_flow::UnitFlow>::new(());
    machine.send(Event::Abort).unwrap();
    assert!(machine.is_done());
    assert_eq!(machine.take_output(), Some(()));
    assert_eq!(machine.take_output(), None);
}
