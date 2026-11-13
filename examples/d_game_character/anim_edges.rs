//! Edge enums of the leaves of the `anim` region (spec 5.2, 5.3): every pose
//! has a `goto!` to each of the other three.

#![allow(clippy::enum_variant_names)]

use estado_core::{Edge, Sink, Slot, StateId};

use super::actions::CharacterCx;
use super::edges::switch_anim;
use super::model::{Anim, Character, Command};

/// Edges of `#[on(Idle)]`.
pub enum IdleEdges {
    /// `goto!(Run)`.
    ToRun,
    /// `goto!(Jump)`.
    ToJump,
    /// `goto!(Shoot)`.
    ToShoot,
}

impl Edge<Character> for IdleEdges {
    const TARGETS: &'static [StateId] =
        &[Character::RUN_ID, Character::JUMP_ID, Character::SHOOT_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToRun => Anim::Run,
            Self::ToJump => Anim::Jump,
            Self::ToShoot => Anim::Shoot,
        };
        switch_anim(slot, cx, next);
        true
    }
}

/// Edges of `#[on(Run)]`.
pub enum RunEdges {
    /// `goto!(Idle)`.
    ToIdle,
    /// `goto!(Jump)`.
    ToJump,
    /// `goto!(Shoot)`.
    ToShoot,
}

impl Edge<Character> for RunEdges {
    const TARGETS: &'static [StateId] =
        &[Character::IDLE_ID, Character::JUMP_ID, Character::SHOOT_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToIdle => Anim::Idle,
            Self::ToJump => Anim::Jump,
            Self::ToShoot => Anim::Shoot,
        };
        switch_anim(slot, cx, next);
        true
    }
}

/// Edges of `#[on(Jump)]`.
pub enum JumpEdges {
    /// `goto!(Idle)`.
    ToIdle,
    /// `goto!(Run)`.
    ToRun,
    /// `goto!(Shoot)`.
    ToShoot,
}

impl Edge<Character> for JumpEdges {
    const TARGETS: &'static [StateId] =
        &[Character::IDLE_ID, Character::RUN_ID, Character::SHOOT_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToIdle => Anim::Idle,
            Self::ToRun => Anim::Run,
            Self::ToShoot => Anim::Shoot,
        };
        switch_anim(slot, cx, next);
        true
    }
}

/// Edges of `#[on(Shoot)]`.
pub enum ShootEdges {
    /// `goto!(Idle)`.
    ToIdle,
    /// `goto!(Run)`.
    ToRun,
    /// `goto!(Jump)`.
    ToJump,
}

impl Edge<Character> for ShootEdges {
    const TARGETS: &'static [StateId] =
        &[Character::IDLE_ID, Character::RUN_ID, Character::JUMP_ID];

    fn apply<S: Sink<Command>>(
        self,
        slot: &mut Slot<Character>,
        cx: &mut CharacterCx<'_, S>,
    ) -> bool {
        let next = match self {
            Self::ToIdle => Anim::Idle,
            Self::ToRun => Anim::Run,
            Self::ToJump => Anim::Jump,
        };
        switch_anim(slot, cx, next);
        true
    }
}
