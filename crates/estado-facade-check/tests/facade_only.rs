//! ADR 0013, item 1: a machine with regions and a completion in a crate whose
//! only dependency is the `estado` facade.
#![allow(missing_docs, dead_code)]

use estado::prelude::*;
use estado_facade_check::{Switch, Toggle};

#[test]
fn a_machine_in_a_facade_only_crate_runs() {
    let mut machine = Machine::<Switch>::new(());
    machine.send(Toggle::Flip).unwrap();
    assert_eq!(machine.path().to_string(), "On");
    machine.send(Toggle::Flip).unwrap();
    assert_eq!(machine.path().to_string(), "Off");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Sent,
    Verified,
}

#[machine(event = Event, ctx = u8)]
mod upload {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Upload {
        #[initial]
        Idle,
        Running(Running),
        #[terminal]
        Finished,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        pub rounds: u8,
        #[region]
        pub transfer: Transfer,
        #[region]
        pub check: Check,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Transfer {
        #[initial]
        Sending,
        #[terminal]
        Sent,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Check {
        #[initial]
        Checking,
        #[terminal]
        Verified,
    }

    #[handlers]
    impl Upload {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Go => goto!(Running { rounds: 0 }),
                _ => bubble!(),
            }
        }

        #[on(Sending)]
        fn sending(event: &Event) -> Next<Self> {
            match event {
                Event::Sent => goto!(Sent),
                _ => bubble!(),
            }
        }

        #[on(Checking)]
        fn checking(event: &Event) -> Next<Self> {
            match event {
                Event::Verified => goto!(Verified),
                _ => bubble!(),
            }
        }

        #[on_done(Running)]
        fn running_done(_up: RunningFields<'_>, cx: &mut Cx<Self>) -> Next<Self> {
            *cx.ctx_mut() += 1;
            goto!(Finished)
        }
    }
}

#[test]
fn regions_completion_and_the_graph_resolve_through_the_facade() {
    use upload::Upload;
    let mut machine = Machine::<Upload>::new(0);
    machine.send(Event::Go).unwrap();
    machine.send(Event::Sent).unwrap();
    machine.send(Event::Verified).unwrap();
    assert!(machine.is_done());
    assert_eq!(*machine.ctx(), 1);
    assert!(<Upload as Spec>::GRAPH.states.len() > 3);
}
