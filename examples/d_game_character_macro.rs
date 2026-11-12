//! Scenario D (game character with a parallel `Alive` state) written with
//! `#[machine]`: the same machine as [`crate::d_game_character`], which is
//! the hand-written code the macro must reproduce (milestone M3). Both
//! versions pass the same tests (`tests/d_game_character.rs`).
//!
//! The events, the context, the sounds and the tuning constants are shared
//! with the hand-written version. `Command` is not: it names the `Anim` of
//! the `anim` region, which is declared inside the machine module, so each
//! version has a `Command` of its own.

mod machine;

pub use crate::d_game_character::{
    DEFER_CAP, DEFER_OVERFLOW_POLICY, Event, FIRE_COOLDOWN, GameCtx, JUMP_TIME, MAGAZINE_SIZE,
    MAX_HEALTH, RAISE_CAP, RELOAD_TIME, Sound,
};
pub use machine::Command;
pub use machine::character::{Alive, AliveFields, Anim, Character, Movement, Weapon};
