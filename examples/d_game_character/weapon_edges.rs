//! Edge enums of the leaves of the `weapon` region (spec 5.2, 5.3).

#![allow(clippy::enum_variant_names)]

use estado_core::{Edge, Sink, Slot, StateId};

use super::actions::CharacterCx;
use super::edges::switch_weapon;
use super::model::{Character, Command, Weapon};

/// Edges of `#[on(Ready)]`.
pub enum ReadyEdges {
    /// `goto!(Firing)` on `Fire`.
    ToFiring,
    /// `goto!(Reloading)` on `ReloadRequest`.
    ToReloading,
}

impl Edge<Character> for ReadyEdges {
    const TARGETS: &'static [StateId] = &[Character::FIRING_ID, Character::RELOADING_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToFiring => Weapon::Firing,
            Self::ToReloading => Weapon::Reloading,
        };
        switch_weapon(slot, cx, next);
        true
    }
}

/// Edges of `#[on(Firing)]`.
pub enum FiringEdges {
    /// `goto!(Ready)` when the cooldown ends with rounds left.
    ToReady,
    /// `goto!(Empty)` when the cooldown ends on the last round.
    ToEmpty,
}

impl Edge<Character> for FiringEdges {
    const TARGETS: &'static [StateId] = &[Character::READY_ID, Character::EMPTY_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToReady => Weapon::Ready,
            Self::ToEmpty => Weapon::Empty,
        };
        switch_weapon(slot, cx, next);
        true
    }
}

/// Edges of `#[on(Reloading)]`.
pub enum ReloadingEdges {
    /// `goto!(Ready)` on `ReloadDone`.
    ToReady,
}

impl Edge<Character> for ReloadingEdges {
    const TARGETS: &'static [StateId] = &[Character::READY_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        match self {
            Self::ToReady => switch_weapon(slot, cx, Weapon::Ready),
        }
        true
    }
}

/// Edges of `#[on(Empty)]`.
pub enum EmptyEdges {
    /// `goto!(Reloading)` on `ReloadRequest`.
    ToReloading,
}

impl Edge<Character> for EmptyEdges {
    const TARGETS: &'static [StateId] = &[Character::RELOADING_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        match self {
            Self::ToReloading => switch_weapon(slot, cx, Weapon::Reloading),
        }
        true
    }
}
