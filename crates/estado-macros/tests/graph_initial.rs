//! Spec errata 19 (resolved by bead 5.5): `Graph::initial` is the variant of
//! the root enum marked `#[initial]`, wherever it is declared.
#![allow(dead_code, missing_docs)]

use estado_core::{Next, Spec, StateId};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
}

#[machine(event = Event, allow(W0001, W0002))]
mod late {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Late {
        Waiting,
        Booting,
        #[initial]
        Running(Running),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        #[substates]
        pub phase: Phase,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Phase {
        Warm,
        #[initial]
        Hot,
    }

    #[handlers]
    impl Late {
        #[on(Waiting)]
        fn waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Booting),
            }
        }
    }
}

use late::Late;

#[test]
fn the_graph_records_an_initial_state_that_is_not_the_first_variant() {
    let graph = <Late as Spec>::GRAPH;
    let running = graph.find("Running").unwrap();
    assert_eq!(graph.initial, Some(running.id));
    assert_ne!(graph.initial, Some(StateId::new(0)));
    assert_eq!(graph.state(running.id).parent, None);
}

#[test]
fn the_initial_state_of_the_graph_is_the_initial_state_of_the_machine() {
    let graph = <Late as Spec>::GRAPH;
    let initial = Late::initial();
    // The compound `Running` starts in its own `#[initial]`, `Hot`.
    let hot = graph.find("Hot").unwrap();
    assert_eq!(graph.state(graph.initial.unwrap()).initial, Some(hot.id));
    assert_eq!(initial.state_id(), hot.id);
}
