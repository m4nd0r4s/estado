//! ADR 0013 item 2: `unhandled!()` applies the nearest `#[unhandled(p)]`
//! override, the state's own or an ancestor's, exactly like `bubble!()`
//! reaching them. Before the fix `unhandled!()` always used the machine
//! policy, so `_ => unhandled!()` in a leaf silently skipped the override of
//! the compound around it.
#![allow(dead_code, missing_docs)]

use estado_core::{Machine, Next, SendError};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Edit,
    Dash,
    Poke,
    Climb,
}

#[machine(event = Event, allow(W0001, W0002))]
mod doc {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Doc {
        #[initial]
        Idle,
        Editing(Editing),
        Dashboard(Dashboard),
    }

    #[state]
    #[unhandled(error)]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Editing {
        pub dirty: bool,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Typing,
        Selecting,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Dashboard {
        pub refreshes: u8,
        #[region]
        pub top: Top,
        #[region]
        pub bottom: Bottom,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Top {
        #[initial]
        #[unhandled(error)]
        TopIdle,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Bottom {
        #[initial]
        BottomIdle,
    }

    #[handlers]
    impl Doc {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Edit => goto!(Editing { dirty: false }),
                Event::Dash => goto!(Dashboard { refreshes: 0 }),
                _ => unhandled!(),
            }
        }

        /// The leaf says `unhandled!()`: the override of `Editing` applies.
        #[on(Typing)]
        fn typing(event: &Event) -> Next<Self> {
            match event {
                Event::Climb => bubble!(),
                _ => unhandled!(),
            }
        }

        /// Inside a region, the state's own override stops the event.
        #[on(TopIdle)]
        fn top_idle(_event: &Event) -> Next<Self> {
            unhandled!()
        }
    }
}

use doc::Doc;

fn started(entry: Event) -> Machine<Doc> {
    let mut machine = Machine::<Doc>::new(());
    machine.send(entry).unwrap();
    machine
}

#[test]
fn unhandled_in_a_leaf_applies_the_override_of_an_ancestor() {
    let mut machine = started(Event::Edit);

    let result = machine.send(Event::Poke);

    assert!(matches!(result, Err(SendError::Unhandled(Event::Poke))));
    assert_eq!(machine.path().to_string(), "Editing/Typing");
}

#[test]
fn bubble_in_a_leaf_reaches_the_same_override() {
    let mut machine = started(Event::Edit);

    let result = machine.send(Event::Climb);

    assert!(matches!(result, Err(SendError::Unhandled(Event::Climb))));
}

#[test]
fn without_an_override_the_machine_policy_applies() {
    let mut machine = Machine::<Doc>::new(());

    let report = machine.send(Event::Poke).unwrap().report;

    assert!(!report.handled);
}

#[test]
fn unhandled_inside_a_region_applies_the_override_of_its_state() {
    let mut machine = started(Event::Dash);

    let result = machine.send(Event::Poke);

    assert!(matches!(result, Err(SendError::Unhandled(Event::Poke))));
}
