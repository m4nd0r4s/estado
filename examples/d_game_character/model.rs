//! The user's own types: states, events, commands and tuning constants
//! (spec 3.2, 3.3).
//!
//! Everything here is `Copy` and a few bytes wide on purpose: events travel by
//! value at game-loop frequency and the machine itself is meant to be
//! instantiated by the thousand (BM-04).

use std::time::Duration;

use estado_core::DeferOverflowPolicy;

/// Health of a fresh character; `Heal` never goes above it.
pub const MAX_HEALTH: u16 = 100;
/// Rounds in a full magazine.
pub const MAGAZINE_SIZE: u8 = 6;
/// How long `Jumping` lasts before the character lands (`#[after]`).
pub const JUMP_TIME: Duration = Duration::from_millis(600);
/// Time between two shots: how long `Firing` lasts (`#[after]`).
pub const FIRE_COOLDOWN: Duration = Duration::from_millis(120);
/// How long `Reloading` takes (`#[after(RELOAD_TIME => ReloadDone)]`).
pub const RELOAD_TIME: Duration = Duration::from_millis(1500);

/// `#[machine(defer_cap = 2)]`: only `ReloadRequest` is ever deferred.
pub const DEFER_CAP: usize = 2;
/// `#[machine(raise_cap = 0)]`: no handler or entry raises an event.
pub const RAISE_CAP: usize = 0;
/// `#[machine(defer_overflow = drop_oldest)]`: a player hammering the reload
/// key in the air must not get an error back. Every deferred event is the
/// same request, so losing the oldest one loses nothing.
pub const DEFER_OVERFLOW_POLICY: DeferOverflowPolicy = DeferOverflowPolicy::DropOldest;

/// The machine: root enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Character {
    /// Parallel state with the regions `movement`, `weapon` and `anim`.
    /// `#[initial]`.
    Alive(Alive),
    /// Terminal: every event is ignored, except `Respawn`.
    Dead,
}

/// Parallel state: shared data and three `#[region]`s, in evaluation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alive {
    /// Hit points; zero is never stored, `Damage` leaves `Alive` instead.
    pub health: u16,
    /// Rounds in the magazine, `0..=MAGAZINE_SIZE`.
    pub ammo: u8,
    /// First region.
    pub movement: Movement,
    /// Second region; reads `movement` through `cx.in_state`.
    pub weapon: Weapon,
    /// Third region; reads both others through `cx.in_state`.
    pub anim: Anim,
}

impl Alive {
    /// `goto!(Alive { .. })`: full health and magazine, each region in its
    /// `#[initial]`.
    pub const fn fresh() -> Self {
        Self {
            health: MAX_HEALTH,
            ammo: MAGAZINE_SIZE,
            movement: Movement::Standing,
            weapon: Weapon::Ready,
            anim: Anim::Idle,
        }
    }
}

/// Region `movement`. The standing state is not called `Idle` because state
/// names are unique across all levels (spec 3.2, E0012) and `anim` has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// `#[initial]`.
    Standing,
    /// Moving on the ground.
    Walking,
    /// In the air; `#[after(JUMP_TIME => Event::Land)]`.
    Jumping,
    /// Ducking.
    Crouching,
}

/// Region `weapon`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weapon {
    /// `#[initial]`; the magazine is never empty here.
    Ready,
    /// A shot was just fired; `#[after(FIRE_COOLDOWN => Event::CooldownElapsed)]`.
    Firing,
    /// `#[after(RELOAD_TIME => Event::ReloadDone)]`.
    Reloading,
    /// The magazine is empty.
    Empty,
}

/// Region `anim`: the pose derived from what the other regions are doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anim {
    /// `#[initial]`.
    Idle,
    /// Walking.
    Run,
    /// In the air.
    Jump,
    /// Firing; wins over every other pose.
    Shoot,
}

/// Events of the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Start walking.
    Move,
    /// Stop walking.
    Stop,
    /// Leave the ground.
    Jump,
    /// The jump timer of `Jumping`.
    Land,
    /// Duck.
    Crouch,
    /// Stand up.
    Stand,
    /// Pull the trigger.
    Fire,
    /// The cooldown timer of `Firing`.
    CooldownElapsed,
    /// Ask for a new magazine: deferred while airborne.
    ReloadRequest,
    /// The reload timer of `Reloading`.
    ReloadDone,
    /// Lose that many hit points; handled by `Alive` itself.
    Damage(u16),
    /// Recover that many hit points; handled by `Alive` itself.
    Heal(u16),
    /// Come back to life; handled by `Dead`.
    Respawn,
}

/// Effects (commands) the machine emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// The pose changed: start this animation.
    PlayAnimation(Anim),
    /// A shot left the barrel.
    SpawnProjectile,
    /// Play a sound.
    PlaySound(Sound),
}

/// Sounds of [`Command::PlaySound`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// A shot was fired.
    Gunshot,
    /// The trigger was pulled on an empty magazine.
    EmptyClick,
    /// A reload started.
    ReloadStart,
    /// The character died.
    Death,
}
