//! Scenario I (media player) as a `#[machine]` module: it must compile, and
//! the generated state id constants must follow the tree (spec 6.5).
#![allow(dead_code, missing_docs, unused_imports)]

use std::time::Duration;

use estado_core::StateId;
use estado_macros::machine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackId(pub u32);

#[derive(Debug, PartialEq, Eq)]
pub struct AudioBuffer {
    samples: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play(TrackId),
    Pause,
    Resume,
    Tick(Duration),
    Stop,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    OpenTrack(TrackId),
    StartOutput,
    StopOutput,
}

#[machine(event = Event, effect = Command, max_microsteps = 32, allow(W0001, W0002))]
/// The media player machine.
pub mod player {
    use super::*;

    /// The machine: root enum.
    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        Failed {
            reason: String,
            buffer: AudioBuffer,
        },
    }

    /// Compound state.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub track: TrackId,
        pub buf: AudioBuffer,
        #[substates]
        pub sub: ActiveSub,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ActiveSub {
        #[initial]
        Playing {
            pos: Duration,
        },
        Paused {
            pos: Duration,
        },
    }

    /// Helper kept as is.
    pub const VOLUME: u8 = 7;

    #[handlers]
    impl Player {}
}

pub use player::Player;

#[test]
fn state_id_constants_follow_the_pre_order_of_the_tree() {
    assert_eq!(Player::IDLE, StateId::new(0));
    assert_eq!(Player::ACTIVE, StateId::new(1));
    assert_eq!(Player::PLAYING, StateId::new(2));
    assert_eq!(Player::PAUSED, StateId::new(3));
    assert_eq!(Player::FAILED, StateId::new(4));
}

#[test]
fn types_and_passthrough_items_survive_expansion() {
    let player = Player::Active(player::Active {
        track: TrackId(1),
        buf: AudioBuffer {
            samples: Vec::new(),
        },
        sub: player::ActiveSub::Playing {
            pos: Duration::ZERO,
        },
    });
    assert!(matches!(player, Player::Active(_)));
    assert_eq!(player::VOLUME, 7);
}
