//! ADR 0013 item 5 and amendment 4: a `goto!` that leaves the active
//! configuration as it was reports `changed = false`, so the deferred events
//! are not requeued (spec 5.4). That holds for a target that is an ancestor of
//! the handler's state and for one that is a descendant of it and already
//! active; a configuration that does change requeues them.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Defer,
    Advance,
    Reset,
    GoToAncestor,
    GoToActiveDescendant,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, doc::Doc, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>, allow(W0001, W0002))]
mod doc {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Doc {
        #[initial]
        Idle,
        Wrap(Wrap),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Wrap {
        #[substates]
        pub inner: WrapInner,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum WrapInner {
        #[initial]
        Both(Both),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Both {
        #[region]
        pub left: Left,
        #[region]
        pub right: Right,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Left {
        #[initial]
        LeftFirst,
        LeftSecond,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Right {
        #[initial]
        RightOnly,
    }

    #[handlers]
    impl Doc {
        #[on(Idle)]
        fn idle(_event: &Event) -> Next<Self> {
            goto!(Wrap)
        }

        #[on(LeftFirst)]
        fn left_first(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Advance => goto!(LeftSecond),
                Event::GoToAncestor => goto!(Wrap),
                Event::Defer => {
                    note(cx, "left sees the event");
                    defer!()
                }
                _ => bubble!(),
            }
        }

        #[on(LeftSecond)]
        fn left_second(event: &Event) -> Next<Self> {
            match event {
                Event::Reset => goto!(Wrap),
                Event::Defer => defer!(),
                _ => bubble!(),
            }
        }

        #[on(RightOnly)]
        fn right_only(event: &Event) -> Next<Self> {
            match event {
                Event::Defer => defer!(),
                _ => bubble!(),
            }
        }

        #[on(Wrap)]
        fn wrap(event: &Event) -> Next<Self> {
            match event {
                Event::GoToActiveDescendant => goto!(Both),
                _ => bubble!(),
            }
        }
    }
}

use doc::Doc;

/// In `Wrap/Both` with a deferred event: both regions defer it, and the left
/// one notes every delivery.
fn with_a_deferred_event() -> Machine<Doc> {
    let mut machine = Machine::<Doc>::new(Vec::new());
    machine.send(Event::Start).unwrap();
    machine.send(Event::Defer).unwrap();
    assert_eq!(machine.ctx(), &["left sees the event".to_owned()]);
    machine
}

fn times_the_left_saw_the_event(machine: &Machine<Doc>) -> usize {
    machine.ctx().len()
}

#[test]
fn a_goto_to_an_ancestor_that_lands_in_the_same_configuration_does_not_requeue() {
    let mut machine = with_a_deferred_event();

    machine.send(Event::GoToAncestor).unwrap();

    assert_eq!(times_the_left_saw_the_event(&machine), 1);
}

#[test]
fn a_goto_to_a_descendant_that_is_already_active_does_not_requeue() {
    let mut machine = with_a_deferred_event();

    machine.send(Event::GoToActiveDescendant).unwrap();

    assert_eq!(times_the_left_saw_the_event(&machine), 1);
}

#[test]
fn a_goto_that_changes_the_configuration_requeues_the_deferred_events() {
    let mut machine = with_a_deferred_event();
    machine.send(Event::Advance).unwrap();

    machine.send(Event::Reset).unwrap();

    assert_eq!(times_the_left_saw_the_event(&machine), 2);
}
