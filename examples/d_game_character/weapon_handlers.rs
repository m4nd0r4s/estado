//! Handlers of the leaves of the `weapon` region (spec 6.2). The second
//! region: it asks `cx.in_state` about `movement`, which ran before it.

use estado_core::{Next, Sink};

use super::actions::{CharacterCx, note};
use super::model::{Character, Command, Event, MAGAZINE_SIZE, Sound};
use super::views::AliveFields;
use super::weapon_edges::{EmptyEdges, FiringEdges, ReadyEdges, ReloadingEdges};

/// `#[on(Ready)]`.
///
/// A reload is refused while the character is airborne by *deferring* the
/// request: landing changes the configuration, so the request is redelivered
/// and the reload starts then, which is what the player meant.
pub fn on_ready<S: Sink<Command>>(
    up: AliveFields<'_>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, ReadyEdges> {
    note(cx, "handler weapon:Ready");
    match event {
        Event::Fire => {
            *up.ammo = up.ammo.saturating_sub(1);
            cx.emit(Command::SpawnProjectile);
            Next::Goto(ReadyEdges::ToFiring)
        }
        Event::ReloadRequest if *up.ammo == MAGAZINE_SIZE => Next::Stay,
        Event::ReloadRequest if cx.in_state(Character::JUMPING_ID) => Next::Defer,
        Event::ReloadRequest => Next::Goto(ReadyEdges::ToReloading),
        _ => Next::Bubble,
    }
}

/// `#[on(Firing)]`: the trigger is locked until the cooldown ends, and a
/// reload waits for it.
pub fn on_firing<S: Sink<Command>>(
    up: AliveFields<'_>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, FiringEdges> {
    note(cx, "handler weapon:Firing");
    match event {
        Event::CooldownElapsed if *up.ammo == 0 => Next::Goto(FiringEdges::ToEmpty),
        Event::CooldownElapsed => Next::Goto(FiringEdges::ToReady),
        Event::ReloadRequest => Next::Defer,
        _ => Next::Bubble,
    }
}

/// `#[on(Reloading)]`. The timer of this state is the reload; leaving it
/// before it fires (fatal damage) cancels it.
pub fn on_reloading<S: Sink<Command>>(
    up: AliveFields<'_>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, ReloadingEdges> {
    note(cx, "handler weapon:Reloading");
    match event {
        Event::ReloadDone => {
            *up.ammo = MAGAZINE_SIZE;
            Next::Goto(ReloadingEdges::ToReady)
        }
        Event::ReloadRequest => Next::Stay,
        _ => Next::Bubble,
    }
}

/// `#[on(Empty)]`: the trigger clicks; a reload follows the airborne rule of
/// `Ready`.
pub fn on_empty<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, EmptyEdges> {
    note(cx, "handler weapon:Empty");
    match event {
        Event::Fire => {
            cx.emit(Command::PlaySound(Sound::EmptyClick));
            Next::Stay
        }
        Event::ReloadRequest if cx.in_state(Character::JUMPING_ID) => Next::Defer,
        Event::ReloadRequest => Next::Goto(EmptyEdges::ToReloading),
        _ => Next::Bubble,
    }
}
