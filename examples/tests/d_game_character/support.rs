//! Shared helpers of the scenario D tests.
#![allow(dead_code)]

use estado_core::{Machine, TimerKey};
use estado_examples::d_game_character::{
    Alive, Character, Event, FIRE_COOLDOWN, GameCtx, MAGAZINE_SIZE,
};
use estado_test::Harness;

/// A started machine with a trace, the trace cleared.
pub fn started() -> Harness<Character> {
    let mut harness = Harness::new(Machine::<Character>::new(GameCtx::traced()));
    harness.start();
    take_trace(&mut harness);
    harness
}

/// Takes the trace kept in the global context.
pub fn take_trace(harness: &mut Harness<Character>) -> Vec<&'static str> {
    harness.machine_mut().ctx_mut().take_trace()
}

/// The trace without the `handler ..` lines: entries and exits.
pub fn transitions_only(trace: Vec<&'static str>) -> Vec<&'static str> {
    trace
        .into_iter()
        .filter(|line| !line.starts_with("handler"))
        .collect()
}

pub fn path(harness: &Harness<Character>) -> String {
    harness.machine().path().to_string()
}

/// The data of `Alive`; panics when the character is dead.
pub fn alive(harness: &Harness<Character>) -> Alive {
    match harness.machine().state() {
        Character::Alive(alive) => *alive,
        Character::Dead => panic!("expected Alive, got Dead"),
    }
}

/// Fires once and waits out the cooldown: one round fewer, back to `Ready`.
pub fn fire_once(harness: &mut Harness<Character>) {
    harness.send(Event::Fire);
    harness.advance(FIRE_COOLDOWN);
}

/// Fires until the magazine is empty; the weapon ends in `Empty`.
pub fn empty_the_magazine(harness: &mut Harness<Character>) {
    for _ in 0..MAGAZINE_SIZE {
        fire_once(harness);
    }
}

/// The pending timer owned by `state`.
pub fn timer_of(harness: &Harness<Character>, state: estado_core::StateId) -> TimerKey {
    harness
        .pending_timers()
        .iter()
        .find(|timer| timer.key.state == state)
        .unwrap_or_else(|| {
            panic!(
                "no pending timer of {state:?}; pending: {:?}",
                harness.pending_timers()
            )
        })
        .key
}

/// A tiny linear congruential generator (Knuth's MMIX constants): the tests
/// that need "random" events stay deterministic and dependency-free.
pub struct Lcg(u64);

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    /// A value in `0..bound`.
    pub fn below(&mut self, bound: u32) -> u32 {
        self.next_u32() % bound
    }
}

/// A pseudo-random event, weighted towards what a player does. Damage is
/// small enough for a character to survive a few hits and rare enough for
/// respawns to be rare too.
pub fn random_event(rng: &mut Lcg) -> Event {
    match rng.below(20) {
        0 => Event::Move,
        1 => Event::Stop,
        2 | 3 => Event::Jump,
        4 => Event::Crouch,
        5 => Event::Stand,
        6..=9 => Event::Fire,
        10 | 11 => Event::ReloadRequest,
        12 => Event::Land,
        13 => Event::ReloadDone,
        14 => Event::CooldownElapsed,
        15 | 16 => Event::Damage(rng.below(30) as u16 + 1),
        17 => Event::Heal(rng.below(40) as u16),
        18 => Event::Respawn,
        _ => Event::Move,
    }
}
