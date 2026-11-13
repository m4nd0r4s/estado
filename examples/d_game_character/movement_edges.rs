//! Edge enums of the leaves of the `movement` region (spec 5.2, 5.3).

#![allow(clippy::enum_variant_names)]

use estado_core::{Edge, Sink, Slot, StateId};

use super::actions::CharacterCx;
use super::edges::switch_movement;
use super::model::{Character, Command, Movement};

/// Edges of `#[on(Standing)]`.
pub enum StandingEdges {
    /// `goto!(Walking)`.
    ToWalking,
    /// `goto!(Jumping)`.
    ToJumping,
    /// `goto!(Crouching)`.
    ToCrouching,
}

impl Edge<Character> for StandingEdges {
    const TARGETS: &'static [StateId] = &[
        Character::WALKING_ID,
        Character::JUMPING_ID,
        Character::CROUCHING_ID,
    ];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToWalking => Movement::Walking,
            Self::ToJumping => Movement::Jumping,
            Self::ToCrouching => Movement::Crouching,
        };
        switch_movement(slot, cx, next);
        true
    }
}

/// Edges of `#[on(Walking)]`.
pub enum WalkingEdges {
    /// `goto!(Standing)`.
    ToStanding,
    /// `goto!(Jumping)`.
    ToJumping,
    /// `goto!(Crouching)`.
    ToCrouching,
}

impl Edge<Character> for WalkingEdges {
    const TARGETS: &'static [StateId] = &[
        Character::STANDING_ID,
        Character::JUMPING_ID,
        Character::CROUCHING_ID,
    ];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToStanding => Movement::Standing,
            Self::ToJumping => Movement::Jumping,
            Self::ToCrouching => Movement::Crouching,
        };
        switch_movement(slot, cx, next);
        true
    }
}

/// Edges of `#[on(Jumping)]`.
pub enum JumpingEdges {
    /// `goto!(Standing)` on `Land`.
    ToStanding,
}

impl Edge<Character> for JumpingEdges {
    const TARGETS: &'static [StateId] = &[Character::STANDING_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        match self {
            Self::ToStanding => switch_movement(slot, cx, Movement::Standing),
        }
        true
    }
}

/// Edges of `#[on(Crouching)]`.
pub enum CrouchingEdges {
    /// `goto!(Standing)` on `Stand`.
    ToStanding,
}

impl Edge<Character> for CrouchingEdges {
    const TARGETS: &'static [StateId] = &[Character::STANDING_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        match self {
            Self::ToStanding => switch_movement(slot, cx, Movement::Standing),
        }
        true
    }
}
