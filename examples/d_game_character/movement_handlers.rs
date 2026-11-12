//! Handlers of the leaves of the `movement` region (spec 6.2). The first
//! region to run, so it never sees what the other two did to the event.

use estado_core::{Next, Sink};

use super::actions::{CharacterCx, note};
use super::model::{Character, Command, Event};
use super::movement_edges::{CrouchingEdges, JumpingEdges, StandingEdges, WalkingEdges};

/// `#[on(Standing)]`.
pub fn on_standing<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, StandingEdges> {
    note(cx, "handler movement:Standing");
    match event {
        Event::Move => Next::Goto(StandingEdges::ToWalking),
        Event::Jump => Next::Goto(StandingEdges::ToJumping),
        Event::Crouch => Next::Goto(StandingEdges::ToCrouching),
        _ => Next::Bubble,
    }
}

/// `#[on(Walking)]`: moving again while walking changes nothing but is
/// handled, so the event is consumed.
pub fn on_walking<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, WalkingEdges> {
    note(cx, "handler movement:Walking");
    match event {
        Event::Move => Next::Stay,
        Event::Stop => Next::Goto(WalkingEdges::ToStanding),
        Event::Jump => Next::Goto(WalkingEdges::ToJumping),
        Event::Crouch => Next::Goto(WalkingEdges::ToCrouching),
        _ => Next::Bubble,
    }
}

/// `#[on(Jumping)]`: no double jump, no air control; only the landing.
pub fn on_jumping<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, JumpingEdges> {
    note(cx, "handler movement:Jumping");
    match event {
        Event::Land => Next::Goto(JumpingEdges::ToStanding),
        _ => Next::Bubble,
    }
}

/// `#[on(Crouching)]`.
pub fn on_crouching<S: Sink<Command>>(
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Next<Character, CrouchingEdges> {
    note(cx, "handler movement:Crouching");
    match event {
        Event::Stand => Next::Goto(CrouchingEdges::ToStanding),
        _ => Next::Bubble,
    }
}
