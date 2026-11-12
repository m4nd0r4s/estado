//! Gaps of the macro tests found by the M3 reviews: `bubble!()` through a
//! compound state that has no handler, a constructor edge whose LCA is below
//! the root, and the order of a constructor against the exits and entries
//! around it, observed through a real side effect of the closure.
#![allow(dead_code, missing_docs)]

use std::cell::RefCell;
use std::rc::Rc;

use estado_core::{Cx, Machine, Next, Sink};
use estado_macros::machine;

type Log = Rc<RefCell<Vec<String>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Poke,
    Shift,
    Park,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, relay::Relay, S>, line: &str) {
    cx.ctx().borrow_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Log, allow(W0001, W0002))]
mod relay {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Relay {
        #[initial]
        Idle,
        Outer(Outer),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Outer {
        pub hops: u8,
        #[substates]
        pub stage: Stage,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Stage {
        #[initial]
        Inner(Inner),
        Parked,
    }

    /// A compound state without any handler: events bubble through it.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Inner {
        #[substates]
        pub step: Step,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Step {
        #[initial]
        Leaf,
        Other {
            n: u8,
        },
    }

    #[handlers]
    impl Relay {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Outer { hops: 0 }),
                _ => bubble!(),
            }
        }

        #[on(Leaf)]
        fn leaf(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Poke => bubble!(),
                Event::Shift => goto!(Other { n: 7 }),
                Event::Park => {
                    let log = cx.ctx().clone();
                    goto!(Parked, move |_old: Exiting![Inner]| {
                        log.borrow_mut().push("constructor".to_owned());
                        Stage::Parked
                    })
                }
                _ => bubble!(),
            }
        }

        #[on(Outer)]
        fn outer(outer: OuterFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Poke => {
                    *outer.hops += 1;
                    note(cx, "outer handles");
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[exit(Leaf)]
        fn exit_leaf(cx: &mut Cx<Self>) {
            note(cx, "exit Leaf");
        }

        #[exit(Inner)]
        fn exit_inner(cx: &mut Cx<Self>) {
            note(cx, "exit Inner");
        }

        #[entry(Parked)]
        fn entry_parked(cx: &mut Cx<Self>) {
            note(cx, "entry Parked");
        }
    }
}

use relay::{Outer, Relay, Stage};

fn running() -> (Machine<Relay>, Log) {
    let log = Log::default();
    let mut machine = Machine::<Relay>::new(log.clone());
    machine.send(Event::Start).unwrap();
    log.borrow_mut().clear();
    (machine, log)
}

#[test]
fn bubble_goes_through_a_compound_state_without_a_handler() {
    let (mut machine, log) = running();

    let report = machine.send(Event::Poke).unwrap().report;

    assert!(report.handled);
    assert_eq!(*log.borrow(), ["outer handles"]);
    let Relay::Outer(Outer { hops, .. }) = machine.state() else {
        panic!("Outer is active");
    };
    assert_eq!(*hops, 1);
}

#[test]
fn a_constructor_runs_after_the_exits_and_before_the_entries() {
    let (mut machine, log) = running();

    machine.send(Event::Park).unwrap();

    assert_eq!(
        *log.borrow(),
        ["exit Leaf", "exit Inner", "constructor", "entry Parked"]
    );
}

#[test]
fn a_constructor_whose_lca_is_below_the_root_keeps_what_is_above_it() {
    let (mut machine, _log) = running();
    machine.send(Event::Poke).unwrap();

    machine.send(Event::Park).unwrap();

    assert_eq!(machine.path().to_string(), "Outer/Parked");
    assert_eq!(
        *machine.state(),
        Relay::Outer(Outer {
            hops: 1,
            stage: Stage::Parked
        })
    );
}
