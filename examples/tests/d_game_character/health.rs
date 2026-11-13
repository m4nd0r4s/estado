// Health: damage and healing are handled by `Alive` itself, after the
// regions declined the event.

use estado_core::IgnoreReason;
use super::scenario::{Character, Event, MAX_HEALTH, RELOAD_TIME};
use estado_test::assert_state;

use super::support::{alive, fire_once, path, started, take_trace, timer_of};

#[test]
fn non_fatal_damage_stays_without_touching_the_regions() {
    let mut harness = started();
    let before = path(&harness);
    let step = harness.send(Event::Damage(30));
    step.expect_no_effect();
    assert!(step.report.handled);
    assert_eq!(step.report.transitions, 0);
    assert_eq!(alive(&harness).health, MAX_HEALTH - 30);
    assert_eq!(path(&harness), before);
    assert!(
        take_trace(&mut harness)
            .iter()
            .all(|line| line.starts_with("handler")),
        "no entry or exit"
    );
}

#[test]
fn non_fatal_damage_does_not_disturb_a_running_reload() {
    let mut harness = started();
    fire_once(&mut harness);
    harness.send(Event::ReloadRequest);
    let reload = timer_of(&harness, Character::RELOADING_ID);

    harness.send(Event::Damage(99));
    assert_eq!(alive(&harness).health, 1);
    assert_eq!(timer_of(&harness, Character::RELOADING_ID), reload);
    harness.advance(RELOAD_TIME);
    assert_eq!(alive(&harness).ammo, 6);
}

#[test]
fn damage_adds_up_and_the_last_point_is_fatal() {
    let mut harness = started();
    for _ in 0..3 {
        harness.send(Event::Damage(33));
    }
    assert_eq!(alive(&harness).health, 1);
    harness.send(Event::Damage(1));
    assert_state!(harness, Character::Dead);
}

#[test]
fn overkill_damage_is_fatal_and_does_not_overflow() {
    let mut harness = started();
    harness.send(Event::Damage(u16::MAX));
    assert_state!(harness, Character::Dead);
}

#[test]
fn zero_damage_does_nothing() {
    let mut harness = started();
    let report = harness.send(Event::Damage(0)).report;
    assert!(report.handled);
    assert_eq!(alive(&harness).health, MAX_HEALTH);
}

#[test]
fn heal_restores_health_up_to_the_maximum() {
    let mut harness = started();
    harness.send(Event::Damage(60));
    harness.send(Event::Heal(25));
    assert_eq!(alive(&harness).health, MAX_HEALTH - 35);
    harness.send(Event::Heal(u16::MAX));
    assert_eq!(alive(&harness).health, MAX_HEALTH);
}

#[test]
fn heal_at_full_health_is_still_a_handled_event() {
    let mut harness = started();
    let report = harness.send(Event::Heal(10)).report;
    assert!(report.handled);
    assert_eq!(report.ignored, None);
    assert_eq!(alive(&harness).health, MAX_HEALTH);
}

#[test]
fn healing_does_not_bring_back_the_dead() {
    let mut harness = started();
    harness.send(Event::Damage(MAX_HEALTH));
    let report = harness.send(Event::Heal(50)).report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert_state!(harness, Character::Dead);
}
