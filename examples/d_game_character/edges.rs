//! Edge enums of the handlers and their transition code (spec 5.2, 5.3).
//!
//! One enum per handler. Every `goto!` of this machine is of the simple form
//! (the target is plain data built in the decision phase), so no variant
//! carries a constructor closure and no enum is generic.
//!
//! This file holds the transition code shared by all of them and the edges of
//! the root states; the three regions have their own files, `movement_edges`,
//! `weapon_edges` and `anim_edges`.
//!
//! No `goto!` of a region leaves `Alive`: the way out is `goto!(Dead)` in the
//! handler of `Alive` itself. The macro knows that statically, so unlike
//! scenario B the dispatch has no "leaves the parallel state" check for the
//! regions (spec 5.6 item 3).

// Variants are named after their targets, so many share a `To` prefix; the
// macro adds the same allow to generated edge enums.
#![allow(clippy::enum_variant_names)]

use estado_core::{Edge, Sink, Slot, StateId};

use super::actions::{
    CharacterCx, enter_anim, enter_movement, enter_root, enter_weapon, exit_anim, exit_movement,
    exit_root, exit_weapon,
};
use super::model::{Alive, Anim, Character, Command, Movement, Weapon};
use super::tasks::{cancel_movement, cancel_weapon, start_movement, start_weapon};

fn alive_mut(slot: &mut Slot<Character>) -> &mut Alive {
    match slot.live_mut() {
        Character::Alive(alive) => alive,
        Character::Dead => unreachable!("Alive is active"),
    }
}

/// Simple-form transition with the root as LCA: the exits (all regions in
/// reverse order when `Alive` is the source) and their cancellations run, the
/// value built in the decision phase is written, the entries, epochs and
/// scheduling run (spec 5.3 steps 1 to 3, 6 to 9; there is no history and no
/// constructor, so steps 4 and 5 do not exist).
pub fn replace_root<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    target: Character,
) {
    exit_root(slot.live_mut(), cx);
    slot.put(target);
    enter_root(slot.live_mut(), cx);
}

/// Transition inside the `movement` region: only the two leaves exit and
/// enter.
pub fn switch_movement<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    next: Movement,
) {
    let alive = alive_mut(slot);
    exit_movement(alive.movement, cx);
    cancel_movement(alive.movement, cx);
    alive.movement = next;
    enter_movement(next, cx);
    start_movement(next, cx);
}

/// Transition inside the `weapon` region.
pub fn switch_weapon<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    next: Weapon,
) {
    let alive = alive_mut(slot);
    exit_weapon(alive.weapon, cx);
    cancel_weapon(alive.weapon, cx);
    alive.weapon = next;
    enter_weapon(next, cx);
    start_weapon(next, cx);
}

/// Transition inside the `anim` region. No leaf of `anim` has a timer.
pub fn switch_anim<S: Sink<Command>>(
    slot: &mut Slot<Character>,
    cx: &mut CharacterCx<'_, S>,
    next: Anim,
) {
    let alive = alive_mut(slot);
    exit_anim(alive.anim, cx);
    alive.anim = next;
    enter_anim(next, cx);
}

/// Edges of `#[on(Alive)]`, the handler of the parallel state itself.
pub enum AliveEdges {
    /// `goto!(Dead)` on a fatal `Damage`: the LCA is the root, so all three
    /// regions exit, in reverse declaration order, and then `Alive`.
    ToDead,
}

impl Edge<Character> for AliveEdges {
    const TARGETS: &'static [StateId] = &[Character::DEAD_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        match self {
            Self::ToDead => replace_root(slot, cx, Character::Dead),
        }
        true
    }
}

/// Edges of `#[on(Dead)]`.
pub enum DeadEdges {
    /// `goto!(Alive(Alive::fresh()))` on `Respawn`.
    ToAlive,
}

impl Edge<Character> for DeadEdges {
    const TARGETS: &'static [StateId] = &[Character::ALIVE_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        match self {
            Self::ToAlive => replace_root(slot, cx, Character::Alive(Alive::fresh())),
        }
        true
    }
}
