//! ADR 0013 item 4: every state that can complete has its own pending flag,
//! so a completion fires exactly once per entry into a final configuration.
//!
//! Before the fix one global flag made `complete` walk the tree again for
//! every terminal entered anywhere: a sibling region completing re-ran the
//! `#[on_done]` of a compound that was already done, two regions with
//! completing compounds did not even compile (E0499), and `on_done` of a
//! parallel state next to a region compound that returned `stay!()` livelocked.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, SendError, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    LeftFinished,
    LeftClose,
    RightFinished,
    RightClose,
    GateClosed,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, job::Job, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>)]
mod job {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Job {
        #[initial]
        Idle,
        Running(Running),
        #[terminal]
        Finished,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        pub id: u8,
        #[region]
        pub left: Left,
        #[region]
        pub right: Right,
        #[region]
        pub gate: Gate,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Left {
        #[initial]
        LeftTask(LeftTask),
        #[terminal]
        LeftOver,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct LeftTask {
        #[substates]
        pub step: LeftStep,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum LeftStep {
        #[initial]
        LeftWorking,
        #[terminal]
        LeftComplete,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Right {
        #[initial]
        RightTask(RightTask),
        #[terminal]
        RightOver,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct RightTask {
        #[substates]
        pub step: RightStep,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum RightStep {
        #[initial]
        RightWorking,
        #[terminal]
        RightComplete,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Gate {
        #[initial]
        Open,
        #[terminal]
        Closed,
    }

    #[handlers]
    impl Job {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Running { id: 1 }),
                _ => bubble!(),
            }
        }

        #[on(LeftWorking)]
        fn left_working(event: &Event) -> Next<Self> {
            match event {
                Event::LeftFinished => goto!(LeftComplete),
                _ => bubble!(),
            }
        }

        #[on(LeftTask)]
        fn left_task(event: &Event) -> Next<Self> {
            match event {
                Event::LeftClose => goto!(LeftOver),
                _ => bubble!(),
            }
        }

        #[on(RightTask)]
        fn right_task(event: &Event) -> Next<Self> {
            match event {
                Event::RightClose => goto!(RightOver),
                _ => bubble!(),
            }
        }

        #[on(RightWorking)]
        fn right_working(event: &Event) -> Next<Self> {
            match event {
                Event::RightFinished => goto!(RightComplete),
                _ => bubble!(),
            }
        }

        #[on(Open)]
        fn open(event: &Event) -> Next<Self> {
            match event {
                Event::GateClosed => goto!(Closed),
                _ => bubble!(),
            }
        }

        /// A done compound whose `on_done` returns `stay!()` must not loop.
        #[on_done(LeftTask)]
        fn left_done(cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, "left done");
            stay!()
        }

        #[on_done(RightTask)]
        fn right_done(cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, "right done");
            stay!()
        }

        #[on_done(Running)]
        fn running_done(cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, "job done");
            goto!(Finished)
        }
    }
}

use job::Job;

fn lines(machine: &mut Machine<Job>) -> Vec<String> {
    std::mem::take(machine.ctx_mut())
}

fn running() -> Machine<Job> {
    let mut machine = Machine::<Job>::new(Vec::new());
    machine.send(Event::Start).unwrap();
    lines(&mut machine);
    machine
}

#[test]
fn two_regions_with_completing_compounds_each_complete_once() {
    let mut machine = running();

    let first = machine.send(Event::LeftFinished).unwrap().report;
    assert_eq!(lines(&mut machine), ["left done"]);
    // The event and the completion; `stay!()` does not loop.
    assert_eq!(first.microsteps, 2);

    // Before the fix the global flag made the left compound complete again
    // here, ahead of the right one.
    let second = machine.send(Event::RightFinished).unwrap().report;
    assert_eq!(lines(&mut machine), ["right done"]);
    assert_eq!(second.microsteps, 2);
    assert!(!machine.is_done());
}

#[test]
fn a_sibling_region_completing_does_not_refire_a_done_compound() {
    let mut machine = running();
    machine.send(Event::LeftFinished).unwrap();
    lines(&mut machine);

    // The gate region reaches a terminal: the completion check of the
    // parallel state runs and finds the other regions still running. The
    // left compound is already done and does not run again.
    let report = machine.send(Event::GateClosed).unwrap().report;

    assert_eq!(lines(&mut machine), Vec::<String>::new());
    assert_eq!(report.microsteps, 2);
}

#[test]
fn the_parallel_on_done_runs_once_after_every_region_closed() {
    let mut machine = running();
    for event in [
        Event::LeftFinished,
        Event::RightFinished,
        Event::GateClosed,
        Event::LeftClose,
        Event::RightClose,
    ] {
        let result = machine.send(event);
        assert!(!matches!(result, Err(SendError::Livelock { .. })));
        result.unwrap();
    }

    assert_eq!(lines(&mut machine), ["left done", "right done", "job done"]);
    assert!(machine.is_done());
}
