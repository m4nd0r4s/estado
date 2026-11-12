// No reload while airborne: `weapon` asks `cx.in_state` about `movement` and
// defers the request, which lands again when the configuration changes
// (spec 5.5, 5.6).

use super::scenario::{
    Anim, Character, Command, DEFER_CAP, Event, FIRE_COOLDOWN, JUMP_TIME, MAGAZINE_SIZE, Movement,
    RELOAD_TIME, Sound, Weapon,
};
use estado_test::{DurationExt, Harness};

use super::support::{alive, empty_the_magazine, fire_once, started, take_trace, timer_of};

/// Standing, one round spent (a full magazine would not need a reload), then
/// in the air.
fn jumping_with_a_spent_round() -> Harness<Character> {
    let mut harness = started();
    fire_once(&mut harness);
    harness.send(Event::Jump);
    take_trace(&mut harness);
    harness
}

#[test]
fn a_reload_request_in_the_air_is_deferred_and_starts_on_landing() {
    let mut harness = jumping_with_a_spent_round();

    let step = harness.send(Event::ReloadRequest);
    step.expect_no_effect();
    assert!(step.report.handled, "deferred is handled");
    assert_eq!(step.report.transitions, 0);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Jumping",
            "handler weapon:Ready",
            "handler anim:Jump",
        ]
    );
    // Only the jump timer: nothing started reloading.
    assert_eq!(harness.pending_timers().len(), 1);

    harness.advance(JUMP_TIME - 1.millis());
    assert_eq!(alive(&harness).weapon, Weapon::Ready, "still in the air");

    // Landing changes the configuration, the request is redelivered and now
    // `in_state(Jumping)` is false: the second microstep starts the reload.
    let step = harness.advance(1.millis());
    assert_eq!(
        step.effects,
        [
            Command::PlayAnimation(Anim::Idle),
            Command::PlaySound(Sound::ReloadStart),
        ]
    );
    assert_eq!(step.report.microsteps, 2);
    assert_eq!(alive(&harness).movement, Movement::Standing);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
    let reload = timer_of(&harness, Character::RELOADING);
    assert_eq!(harness.pending_timers().len(), 1);
    assert_eq!(
        harness.pending_timers()[0].deadline,
        FIRE_COOLDOWN + JUMP_TIME + RELOAD_TIME,
        "{reload:?} starts at landing"
    );

    harness.advance(RELOAD_TIME);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE);
}

#[test]
fn landing_re_enables_reload() {
    let mut harness = jumping_with_a_spent_round();
    harness.advance(JUMP_TIME);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);

    let step = harness.send(Event::ReloadRequest);
    assert_eq!(step.effects, [Command::PlaySound(Sound::ReloadStart)]);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
}

#[test]
fn hammering_the_reload_key_in_the_air_never_fails_and_reloads_once() {
    let mut harness = jumping_with_a_spent_round();
    for _ in 0..(DEFER_CAP * 3) {
        let report = harness.send(Event::ReloadRequest).report;
        assert!(report.handled);
    }
    harness.advance(JUMP_TIME);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
    assert_eq!(harness.pending_timers().len(), 1, "a single reload timer");

    harness.advance(RELOAD_TIME);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE);
    harness.assert_no_pending_timers();
}

#[test]
fn an_empty_weapon_follows_the_same_rule() {
    let mut harness = started();
    empty_the_magazine(&mut harness);
    harness.send(Event::Jump);

    harness.send(Event::ReloadRequest);
    assert_eq!(alive(&harness).weapon, Weapon::Empty);
    harness.advance(JUMP_TIME);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
}

#[test]
fn a_reload_in_progress_survives_a_jump() {
    let mut harness = started();
    fire_once(&mut harness);
    harness.send(Event::ReloadRequest);
    harness.send(Event::Jump);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);

    harness.advance(RELOAD_TIME);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE);
}

#[test]
fn a_reload_requested_mid_shot_waits_for_the_cooldown() {
    let mut harness = started();
    harness.send(Event::Fire);
    let report = harness.send(Event::ReloadRequest).report;
    assert!(report.handled);
    assert_eq!(alive(&harness).weapon, Weapon::Firing);

    let step = harness.advance(FIRE_COOLDOWN);
    assert!(
        step.effects
            .contains(&Command::PlaySound(Sound::ReloadStart))
    );
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
}

#[test]
fn the_deferred_request_is_re_deferred_by_every_change_while_still_airborne() {
    let mut harness = jumping_with_a_spent_round();
    harness.send(Event::ReloadRequest);

    // Firing changes the configuration: the request is redelivered, `Firing`
    // defers it again; the cooldown ends, `Ready` finds the character still
    // in the air and defers it once more.
    harness.send(Event::Fire);
    assert_eq!(alive(&harness).weapon, Weapon::Firing);
    harness.advance(FIRE_COOLDOWN);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    assert_eq!(alive(&harness).movement, Movement::Jumping);

    harness.advance(JUMP_TIME);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
}
