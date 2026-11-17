//! Spec 8.3: `Root::WORST_CASE_ACTIONS` (ADR 0019) is the bound of
//! `Graph::worst_case_actions`, raised to the cost of every `reenter!()`.
#![allow(dead_code, missing_docs)]

use estado_core::{Next, Spec};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Wake,
    Spawn,
    Stop,
    Die,
    Refresh,
}

#[machine(event = Event, allow(W0001, W0002))]
mod cost {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Cost {
        #[initial]
        Idle,
        Active(Active),
        Alive(Alive),
        #[terminal]
        Dead,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Paused,
        Playing(Playing),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Playing {
        #[substates]
        pub speed: Speed,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Speed {
        #[initial]
        Normal,
        Fast,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Alive {
        #[region]
        pub movement: Movement,
        #[region]
        pub weapon: Weapon,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Movement {
        #[initial]
        Standing,
        Walking,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Weapon {
        #[initial]
        Ready,
        Reloading,
    }

    #[handlers]
    impl Cost {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                // Idle exits; Active and Paused enter.
                Event::Wake => goto!(Active),
                // Idle exits; Alive, Standing and Ready enter.
                Event::Spawn => goto!(Alive),
                _ => bubble!(),
            }
        }

        #[on(Normal)]
        fn normal(event: &Event) -> Next<Self> {
            match event {
                // Normal, Playing and Active exit; Idle enters.
                Event::Stop => goto!(Idle),
                _ => bubble!(),
            }
        }

        #[on(Walking)]
        fn walking(event: &Event) -> Next<Self> {
            match event {
                // Walking, Ready (the other region) and Alive exit; Dead enters.
                Event::Die => goto!(Dead),
                _ => bubble!(),
            }
        }
    }
}

#[machine(event = Event, allow(W0001, W0002))]
mod refresh {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Refresher {
        #[initial]
        Idle,
        Alive(Alive),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Alive {
        #[region]
        pub movement: Movement,
        #[region]
        pub weapon: Weapon,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Movement {
        #[initial]
        Standing,
        Walking,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Weapon {
        #[initial]
        Ready,
        Reloading,
    }

    #[handlers]
    impl Refresher {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                // Idle exits; Alive, Standing and Ready enter.
                Event::Spawn => goto!(Alive),
                _ => bubble!(),
            }
        }

        #[on(Alive)]
        fn alive(event: &Event) -> Next<Self> {
            match event {
                // Alive, Standing and Ready exit and enter again.
                Event::Refresh => reenter!(),
                _ => bubble!(),
            }
        }
    }
}

use cost::Cost;
use refresh::Refresher;

#[test]
fn the_constant_is_the_bound_of_the_graph_when_no_handler_reenters() {
    assert_eq!(Cost::WORST_CASE_ACTIONS, 4);
    assert_eq!(Cost::WORST_CASE_ACTIONS, Cost::GRAPH.worst_case_actions());
}

#[test]
fn the_constant_can_be_checked_at_compile_time() {
    const _: () = assert!(Cost::WORST_CASE_ACTIONS <= 4);
}

#[test]
fn a_reenter_counts_although_it_is_not_an_edge() {
    assert_eq!(Refresher::GRAPH.worst_case_actions(), 4);
    assert_eq!(Refresher::WORST_CASE_ACTIONS, 6);
}
