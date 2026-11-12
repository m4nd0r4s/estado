//! Scenario I (media player) written with `#[machine]`: the same machine as
//! [`crate::i_media_player`], which is the hand-written code the macro must
//! reproduce (milestone M3). Both versions pass the same tests
//! (`tests/i_media_player.rs`).
//!
//! The events, commands and the audio buffer are shared with the hand-written
//! version; the states, the handlers and everything generated live in the
//! [`machine`] module.

mod machine;

pub use crate::i_media_player::{AudioBuffer, Command, Event, TrackId};
pub use machine::player::{
    Active, ActiveFields, ActiveSub, FailedExiting, FailedMut, PausedMut, Player, PlayingMut,
};
