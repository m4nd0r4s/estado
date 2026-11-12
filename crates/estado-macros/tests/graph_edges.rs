//! ADR 0013 / review item 19: the static graph keeps `goto!(T)` and
//! `goto!(T, history)` of one `match` arm as two edges. Before the fix the
//! deduplication key ignored the kind of the edge and merged them.
#![allow(dead_code, missing_docs)]

use estado_core::{EdgeKind, Next, Spec};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Resume(bool),
}

#[machine(event = Event, allow(W0001, W0002))]
mod player {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
    }

    #[state]
    #[history]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Normal,
        Fast,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn idle(event: &Event) -> Next<Self> {
            match event {
                Event::Resume(fresh) => {
                    if *fresh {
                        goto!(Active)
                    } else {
                        goto!(Active, history)
                    }
                }
            }
        }
    }
}

use player::Player;

#[test]
fn a_plain_goto_and_a_history_goto_to_the_same_state_are_two_edges() {
    let kinds: Vec<EdgeKind> = Player::GRAPH.edges.iter().map(|edge| edge.kind).collect();

    assert_eq!(kinds, [EdgeKind::Normal, EdgeKind::History]);
}
