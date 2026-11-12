//! A fatal `Damage` preempts everything: all regions exit in reverse
//! declaration order and take their timers with them (spec 5.3, 7.1).

use estado_core::{Effect, IgnoreReason};
use estado_examples::d_game_character::{
    Anim, Character, Command, Event, MAGAZINE_SIZE, MAX_HEALTH, Sound,
};
use estado_test::{DurationExt, Harness, assert_state};

use super::support::{alive, fire_once, path, started, take_trace, timer_of};

/// Jumping, reloading, and so with a timer in two regions.
fn jumping_and_reloading() -> Harness<Character> {
    let mut harness = started();
    fire_once(&mut harness);
    harness.send(Event::ReloadRequest);
    harness.send(Event::Jump);
    take_trace(&mut harness);
    assert_eq!(harness.pending_timers().len(), 2);
    harness
}

#[test]
fn a_fatal_damage_leaves_all_regions_in_reverse_declaration_order() {
    let mut harness = jumping_and_reloading();
    harness.send(Event::Damage(MAX_HEALTH));

    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Jumping",
            "handler weapon:Reloading",
            "handler anim:Jump",
            "handler Alive",
            "exit anim:Jump",
            "exit weapon:Reloading",
            "exit movement:Jumping",
            "exit Alive",
            "entry Dead",
        ]
    );
    assert_state!(harness, Character::Dead);
    assert_eq!(path(&harness), "Dead");
}

#[test]
fn a_fatal_damage_cancels_the_reload_and_leaves_no_timer_behind() {
    let mut harness = jumping_and_reloading();
    harness.send(Event::Damage(MAX_HEALTH));
    harness.assert_no_pending_timers();
    assert!(harness.pending_invokes().is_empty());

    // Nothing wakes up later: no landing, no reload.
    let step = harness.advance(10.secs());
    step.expect_no_effect();
    assert_eq!(step.report.microsteps, 0);
    assert_state!(harness, Character::Dead);
}

#[test]
fn cancellations_follow_the_exit_order_and_the_death_sound_comes_last() {
    let mut harness = jumping_and_reloading();
    let reload = timer_of(&harness, Character::RELOADING);
    let jump = timer_of(&harness, Character::JUMPING);

    let mut effects = Vec::new();
    harness
        .machine_mut()
        .send_with(Event::Damage(MAX_HEALTH), |effect| effects.push(effect))
        .unwrap();
    assert_eq!(
        effects,
        [
            Effect::CancelTimer(reload),
            Effect::CancelTimer(jump),
            Effect::User(Command::PlaySound(Sound::Death)),
        ]
    );
}

#[test]
fn a_late_timer_of_a_state_that_died_is_stale() {
    let mut harness = jumping_and_reloading();
    let reload = timer_of(&harness, Character::RELOADING);
    let jump = timer_of(&harness, Character::JUMPING);
    harness.send(Event::Damage(MAX_HEALTH));

    // The runtime did not cancel in time: the core discards them anyway.
    for key in [reload, jump] {
        let step = harness.machine_mut().fire(key).unwrap();
        assert_eq!(step.report.ignored, Some(IgnoreReason::Stale));
    }
    assert_state!(harness, Character::Dead);
}

#[test]
fn a_fatal_damage_during_a_shot_cancels_the_cooldown() {
    let mut harness = started();
    harness.send(Event::Fire);
    assert_eq!(harness.pending_timers().len(), 1);
    harness.send(Event::Damage(MAX_HEALTH));
    harness.assert_no_pending_timers();
}

#[test]
fn a_deferred_reload_does_not_outlive_the_character() {
    let mut harness = started();
    fire_once(&mut harness);
    harness.send(Event::Jump);
    harness.send(Event::ReloadRequest);

    // The fatal damage changes the configuration: the deferred request is
    // redelivered to `Dead`, which has no use for it.
    let report = harness.send(Event::Damage(MAX_HEALTH)).report;
    assert_eq!(report.unhandled_internal, 1);
    harness.assert_no_pending_timers();

    let report = harness.send(Event::Respawn).report;
    assert_eq!(report.unhandled_internal, 0, "nothing left to redeliver");
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE);
}

#[test]
fn dead_ignores_everything_but_respawn() {
    let mut harness = started();
    harness.send(Event::Damage(MAX_HEALTH));
    take_trace(&mut harness);

    for event in [
        Event::Move,
        Event::Jump,
        Event::Fire,
        Event::ReloadRequest,
        Event::Damage(5),
        Event::Heal(50),
    ] {
        let report = harness.send(event).report;
        assert_eq!(report.ignored, Some(IgnoreReason::Unhandled), "{event:?}");
    }
    assert_eq!(
        take_trace(&mut harness).len(),
        6,
        "only `handler Dead` lines"
    );
    assert_state!(harness, Character::Dead);
}

#[test]
fn respawn_enters_a_fresh_alive_in_new_epochs() {
    let mut harness = jumping_and_reloading();
    let old_reload = timer_of(&harness, Character::RELOADING);
    harness.send(Event::Damage(MAX_HEALTH));
    take_trace(&mut harness);

    let step = harness.send(Event::Respawn);
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Idle)]);
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler Dead",
            "exit Dead",
            "entry Alive",
            "entry movement:Standing",
            "entry weapon:Ready",
            "entry anim:Idle",
        ]
    );
    assert_eq!(alive(&harness).health, MAX_HEALTH);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE);
    harness.assert_no_pending_timers();

    // A new reload belongs to a new instance of `Reloading`.
    fire_once(&mut harness);
    harness.send(Event::ReloadRequest);
    let new_reload = timer_of(&harness, Character::RELOADING);
    assert_eq!(new_reload.epoch, old_reload.epoch + 1);
    let stale = harness.machine_mut().fire(old_reload).unwrap();
    assert_eq!(stale.report.ignored, Some(IgnoreReason::Stale));
}
