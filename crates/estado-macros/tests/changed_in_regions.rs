//! ADR 0013 item 5: `changed` compares the full active configuration for
//! `reenter!()` and for transitions that re-enter a state holding regions, so
//! a re-entry that resets a region requeues the deferred events (spec 5.4).
//!
//! Before the fix `reenter!()` of a state inside a region, and
//! `goto!(Parallel, external)` from inside it, compared the leaf id of the
//! whole machine, which is the parallel state itself: `changed` was always
//! false and the deferred event stayed in the queue.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Edit,
    Save,
    Refresh,
    Restart,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, panel::Panel, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>, allow(W0002))]
mod panel {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Panel {
        #[initial]
        Idle,
        Console(Console),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Console {
        pub id: u8,
        #[region]
        pub left: Left,
        #[region]
        pub right: Right,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Left {
        #[initial]
        Pane(Pane),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Pane {
        #[substates]
        pub view: View,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum View {
        #[initial]
        Browsing,
        Editing,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Right {
        #[initial]
        Steady,
    }

    #[handlers]
    impl Panel {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Console { id: 1 }),
                _ => bubble!(),
            }
        }

        #[on(Browsing)]
        fn browsing(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Edit => goto!(Editing),
                Event::Save => {
                    note(cx, "saved");
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[on(Editing)]
        fn editing(event: &Event) -> Next<Self> {
            match event {
                Event::Save => defer!(),
                Event::Restart => goto!(Console { id: 2 }, external),
                _ => bubble!(),
            }
        }

        #[on(Pane)]
        fn pane(event: &Event) -> Next<Self> {
            match event {
                Event::Refresh => reenter!(),
                _ => bubble!(),
            }
        }
    }
}

use panel::Panel;

fn saving_in_the_editor() -> Machine<Panel> {
    let mut machine = Machine::<Panel>::new(Vec::new());
    for event in [Event::Start, Event::Edit, Event::Save] {
        machine.send(event).unwrap();
    }
    assert!(machine.ctx().is_empty(), "the save is deferred");
    machine
}

#[test]
fn reenter_of_a_state_in_a_region_requeues_the_deferred_events() {
    let mut machine = saving_in_the_editor();

    let report = machine.send(Event::Refresh).unwrap().report;

    assert!(report.transitions >= 1);
    assert_eq!(machine.ctx(), &["saved".to_owned()]);
}

#[test]
fn an_external_self_transition_of_the_parallel_state_requeues_the_deferred_events() {
    let mut machine = saving_in_the_editor();

    machine.send(Event::Restart).unwrap();

    assert_eq!(machine.ctx(), &["saved".to_owned()]);
}
