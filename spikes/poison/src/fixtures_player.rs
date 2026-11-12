//! Nested compound root, as in spec 3.2.

use crate::api::Spec;

#[derive(Debug, PartialEq)]
pub enum Player {
    Idle,
    Active(Active),
    Failed { reason: String },
}

#[derive(Debug, PartialEq)]
pub struct Active {
    pub track: u32,
    pub buf: Vec<u8>,
    pub sub: ActiveSub,
}

#[derive(Debug, PartialEq)]
pub enum ActiveSub {
    Playing { pos: u64 },
    Paused { pos: u64 },
}

impl Spec for Player {
    fn initial() -> Self {
        Player::Idle
    }

    fn next(self) -> Self {
        match self {
            Player::Idle => Player::Active(Active {
                track: 1,
                buf: vec![0; 4],
                sub: ActiveSub::Playing { pos: 0 },
            }),
            Player::Active(_) => Player::Failed {
                reason: "boom".to_string(),
            },
            Player::Failed { .. } => Player::Idle,
        }
    }

    /// The LCA is `Active`: the root is destructured by value down to the
    /// child of the LCA (`sub`), the constructor runs, the path is rebuilt.
    fn nested(self, panic_inside: bool) -> Self {
        let Player::Active(Active { track, buf, sub }) = self else {
            return self.next();
        };
        assert!(!panic_inside, "panic inside nested constructor");
        let sub = match sub {
            ActiveSub::Playing { pos } => ActiveSub::Paused { pos },
            ActiveSub::Paused { pos } => ActiveSub::Playing { pos },
        };
        Player::Active(Active { track, buf, sub })
    }
}
