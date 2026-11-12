//! Decision phase of `Character`: dispatch of the root states, the sequential
//! evaluation of the three regions of `Alive` (spec 5.1, 5.6) and the
//! resolution of a `Next` into a transition (spec 5.3).
//!
//! # Open item 13.4: regions, sequential or "all see the event"
//!
//! The regions run in declaration order, `movement`, `weapon`, `anim`, and each
//! region-internal transition is applied before the next region looks at the
//! event (spec 5.6 items 1 and 2), followed by `cx.refresh_active` (ADR 0005).
//! The decision is ADR 0008; this is where it shows in this scenario:
//!
//! - `anim` derives its pose from `movement` and `weapon` through
//!   `cx.in_state`. It needs the *applied* configuration of the regions that
//!   ran before it: with "all see the event" it would be one event behind, or
//!   each pose handler would have to repeat the guards of the other regions
//!   (is the magazine empty? is the trigger locked?).
//! - `weapon` reads `movement` for the airborne rule. `movement` ran first, so
//!   it reads the applied configuration too; `ReloadRequest` is not handled by
//!   `movement`, so there the two semantics agree.
//! - `Damage` and `Heal` are handled by no region and fall through to the
//!   handler of `Alive` (item 5). That works because no region consumes them:
//!   one `stay!()` for `Damage` in a region and the damage is lost. The same
//!   rule is why no region handles `ReloadRequest` besides `weapon`: a
//!   `Defer` of `weapon` is dropped when another region consumes the event
//!   (item 4), which here would lose a reload the player asked for.
//! - Item 3, a region leaving `Alive`, never happens: the preemption is
//!   `goto!(Dead)` in the handler of `Alive`.

use estado_core::{Edge, Next, Outcome, Sink, Slot, Spec};

use super::actions::CharacterCx;
use super::anim_handlers::{on_idle, on_jump, on_run, on_shoot};
use super::handlers::{on_alive, on_dead};
use super::model::{Alive, Anim, Character, Command, Event, Movement, Weapon};
use super::movement_handlers::{on_crouching, on_jumping, on_standing, on_walking};
use super::weapon_handlers::{on_empty, on_firing, on_ready, on_reloading};

/// What the regions did with one event, for the verdict of spec 5.6.
#[derive(Default)]
struct Tally {
    consumed: bool,
    deferred: bool,
    transitioned: bool,
}

/// Applies `next` for a handler of the root state or of `Alive` itself.
fn resolve<Edges: Edge<Character>, S: Sink<Command>>(
    next: Next<Character, Edges>,
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
) -> Outcome {
    match next {
        Next::Stay => Outcome::Stayed,
        Next::Bubble | Next::Unhandled => Outcome::Unhandled(Character::UNHANDLED),
        Next::Defer => Outcome::Deferred,
        Next::Reenter => unreachable!("no handler of Character returns reenter!()"),
        Next::Goto(edge) => Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        },
        Next::__Never(_, never) => match never {},
    }
}

/// Folds the decision of one region into the tally and applies it at once
/// (spec 5.6 item 2).
///
/// `is_last_region` is known statically by the macro: nobody evaluated after
/// the last region, so the snapshot behind `in_state` is not refreshed for it
/// (the `StatePath` rebuild is the only cost of ADR 0005 on the hot path).
fn absorb<Edges: Edge<Character>, S: Sink<Command>>(
    next: Next<Character, Edges>,
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    tally: &mut Tally,
    is_last_region: bool,
) {
    match next {
        Next::Stay => tally.consumed = true,
        Next::Defer => tally.deferred = true,
        Next::Bubble | Next::Unhandled => {}
        Next::Goto(edge) => {
            tally.consumed = true;
            tally.transitioned = true;
            edge.apply(slot, cx);
            // The transition is applied: the next region must see it through
            // `cx.in_state` (ADR 0005).
            if !is_last_region {
                cx.refresh_active(slot.live_mut().path());
            }
        }
        Next::Reenter => unreachable!("no handler of Character returns reenter!()"),
        Next::__Never(_, never) => match never {},
    }
}

fn alive_mut(slot: &mut Slot<Character>) -> &mut Alive {
    match slot.live_mut() {
        Character::Alive(alive) => alive,
        Character::Dead => unreachable!("Alive is active"),
    }
}

/// Dispatch of the leaf of the `movement` region.
fn evaluate_movement<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
    tally: &mut Tally,
) {
    match alive_mut(slot).movement {
        Movement::Standing => absorb(on_standing(cx, event), slot, cx, tally, false),
        Movement::Walking => absorb(on_walking(cx, event), slot, cx, tally, false),
        Movement::Jumping => absorb(on_jumping(cx, event), slot, cx, tally, false),
        Movement::Crouching => absorb(on_crouching(cx, event), slot, cx, tally, false),
    }
}

/// Dispatch of the leaf of the `weapon` region.
fn evaluate_weapon<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
    tally: &mut Tally,
) {
    let alive = alive_mut(slot);
    let weapon = alive.weapon;
    let (up, _, _, _) = alive.split();
    match weapon {
        Weapon::Ready => absorb(on_ready(up, cx, event), slot, cx, tally, false),
        Weapon::Firing => absorb(on_firing(up, cx, event), slot, cx, tally, false),
        Weapon::Reloading => absorb(on_reloading(up, cx, event), slot, cx, tally, false),
        Weapon::Empty => absorb(on_empty(cx, event), slot, cx, tally, false),
    }
}

/// Dispatch of the leaf of the `anim` region.
fn evaluate_anim<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
    tally: &mut Tally,
) {
    match alive_mut(slot).anim {
        Anim::Idle => absorb(on_idle(cx, event), slot, cx, tally, true),
        Anim::Run => absorb(on_run(cx, event), slot, cx, tally, true),
        Anim::Jump => absorb(on_jump(cx, event), slot, cx, tally, true),
        Anim::Shoot => absorb(on_shoot(cx, event), slot, cx, tally, true),
    }
}

/// Spec 5.6: sequential evaluation. The short-circuit of item 3 does not
/// exist here, because no region transition leaves `Alive`.
fn dispatch_alive<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Outcome {
    let mut tally = Tally::default();
    evaluate_movement(slot, cx, event, &mut tally);
    evaluate_weapon(slot, cx, event, &mut tally);
    evaluate_anim(slot, cx, event, &mut tally);
    if tally.consumed {
        return if tally.transitioned {
            Outcome::Transitioned { changed: true }
        } else {
            Outcome::Stayed
        };
    }
    if tally.deferred {
        return Outcome::Deferred;
    }
    let (up, _, _, _) = alive_mut(slot).split();
    let next = on_alive(up, cx, event);
    resolve(next, slot, cx)
}

/// One microstep of the decision phase: dispatch from the active leaf up.
pub fn dispatch<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    event: &Event,
) -> Outcome {
    match slot.live_mut() {
        Character::Alive(_) => dispatch_alive(slot, cx, event),
        Character::Dead => {
            let next = on_dead(cx, event);
            resolve(next, slot, cx)
        }
    }
}
