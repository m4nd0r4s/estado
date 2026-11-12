//! Handlers of the leaves of the `anim` region (spec 6.2).
//!
//! `anim` is the last region and owns no state of its own: its pose is a
//! function of what `movement` and `weapon` are doing *after* they handled the
//! same event. Every pose handler answers the events that can change the pose
//! with [`desired_pose`], which asks `cx.in_state` and relies on two rules of
//! the spec:
//!
//! - 5.6 item 1: the regions run in declaration order, so `anim` runs last;
//! - 5.6 item 2 and errata 6: a region-internal transition is applied, and the
//!   snapshot behind `in_state` refreshed, before the next region looks.
//!
//! If all regions saw the event before any was applied (13.4), `anim` would
//! see the previous configuration and always be one event behind.
//!
//! When the pose is already the right one the handler bubbles instead of
//! answering `stay!()`: `anim` did nothing, so it must not claim the event.
//! A `stay!()` would make `Fire` count as handled while the trigger is locked,
//! and would drop a `defer!()` another region returned for the same event
//! (spec 5.6 items 4 and 5).

use estado_core::{Next, Sink};

use super::actions::{CharacterCx, note};
use super::anim_edges::{IdleEdges, JumpEdges, RunEdges, ShootEdges};
use super::model::{Anim, Character, Command, Event};

/// The events after which a pose may have to change: the ones `movement` and
/// `weapon` react to by changing state.
const fn changes_pose(event: &Event) -> bool {
    matches!(
        event,
        Event::Move
            | Event::Stop
            | Event::Jump
            | Event::Land
            | Event::Crouch
            | Event::Stand
            | Event::Fire
            | Event::CooldownElapsed
    )
}

/// The pose the character should have right now: shooting wins over jumping,
/// which wins over running.
fn desired_pose<S: Sink<Command>>(cx: &CharacterCx<'_, S>) -> Anim {
    if cx.in_state(Character::FIRING) {
        Anim::Shoot
    } else if cx.in_state(Character::JUMPING) {
        Anim::Jump
    } else if cx.in_state(Character::WALKING) {
        Anim::Run
    } else {
        Anim::Idle
    }
}

/// `#[on(Idle)]`.
pub fn on_idle<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, IdleEdges> {
    note(cx, "handler anim:Idle");
    if !changes_pose(event) {
        return Next::Bubble;
    }
    match desired_pose(cx) {
        Anim::Idle => Next::Bubble,
        Anim::Run => Next::Goto(IdleEdges::ToRun),
        Anim::Jump => Next::Goto(IdleEdges::ToJump),
        Anim::Shoot => Next::Goto(IdleEdges::ToShoot),
    }
}

/// `#[on(Run)]`.
pub fn on_run<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, RunEdges> {
    note(cx, "handler anim:Run");
    if !changes_pose(event) {
        return Next::Bubble;
    }
    match desired_pose(cx) {
        Anim::Run => Next::Bubble,
        Anim::Idle => Next::Goto(RunEdges::ToIdle),
        Anim::Jump => Next::Goto(RunEdges::ToJump),
        Anim::Shoot => Next::Goto(RunEdges::ToShoot),
    }
}

/// `#[on(Jump)]`.
pub fn on_jump<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, JumpEdges> {
    note(cx, "handler anim:Jump");
    if !changes_pose(event) {
        return Next::Bubble;
    }
    match desired_pose(cx) {
        Anim::Jump => Next::Bubble,
        Anim::Idle => Next::Goto(JumpEdges::ToIdle),
        Anim::Run => Next::Goto(JumpEdges::ToRun),
        Anim::Shoot => Next::Goto(JumpEdges::ToShoot),
    }
}

/// `#[on(Shoot)]`.
pub fn on_shoot<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, ShootEdges> {
    note(cx, "handler anim:Shoot");
    if !changes_pose(event) {
        return Next::Bubble;
    }
    match desired_pose(cx) {
        Anim::Shoot => Next::Bubble,
        Anim::Idle => Next::Goto(ShootEdges::ToIdle),
        Anim::Run => Next::Goto(ShootEdges::ToRun),
        Anim::Jump => Next::Goto(ShootEdges::ToJump),
    }
}
