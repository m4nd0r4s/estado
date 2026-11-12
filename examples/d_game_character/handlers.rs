//! The user's handlers of the root states, as they look after `#[handlers]`
//! rewrote each `goto!` into a variant of the handler's edge enum (spec 6.2,
//! 6.3, ADR 0001). The regions' handlers are in `movement_handlers`,
//! `weapon_handlers` and `anim_handlers`.
//!
//! Parameters come in the order of spec 6.2: own data, the `*Fields` of the
//! ancestors, `cx`, the event.

use estado_core::{Next, Sink};

use super::actions::{CharacterCx, note};
use super::edges::{AliveEdges, DeadEdges};
use super::model::{Character, Command, Event, MAX_HEALTH};
use super::views::AliveFields;

/// `#[on(Alive)]`: reached only when no region consumed the event. No region
/// handles `Damage` or `Heal`, so they always land here; a region that
/// answered `Damage` with `stay!()` would swallow it.
///
/// Fatal damage is the preemption of the scenario: `goto!(Dead)` has the root
/// as its LCA, so every region exits, in reverse declaration order, and every
/// timer of the regions is cancelled, the reload included.
pub fn on_alive<S: Sink<Command>>(
    up: AliveFields<'_>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, AliveEdges> {
    note(cx, "handler Alive");
    match event {
        Event::Damage(amount) => {
            *up.health = up.health.saturating_sub(*amount);
            if *up.health == 0 {
                Next::Goto(AliveEdges::ToDead)
            } else {
                Next::Stay
            }
        }
        Event::Heal(amount) => {
            *up.health = up.health.saturating_add(*amount).min(MAX_HEALTH);
            Next::Stay
        }
        _ => Next::Bubble,
    }
}

/// `#[on(Dead)]`: only `Respawn` brings the character back.
pub fn on_dead<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, DeadEdges> {
    note(cx, "handler Dead");
    match event {
        Event::Respawn => Next::Goto(DeadEdges::ToAlive),
        _ => Next::Bubble,
    }
}
