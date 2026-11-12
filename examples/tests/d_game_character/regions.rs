// The regions of `Alive`: evaluation order, `cx.in_state` between them and
// the pose of `anim` derived from the others (spec 5.3, 5.6, ADR 0005,
// ADR 0008).

use super::scenario::{
    Anim, Command, Event, FIRE_COOLDOWN, JUMP_TIME, Movement, RELOAD_TIME, Weapon,
};
use estado_test::DurationExt;

use super::support::{alive, fire_once, path, started, take_trace, timer_of};
use super::scenario::Character;

#[test]
fn regions_are_evaluated_in_declaration_order_and_each_transition_is_applied_at_once() {
    let mut harness = started();
    let step = harness.send(Event::Jump);
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Jump)]);
    // Two regions transitioned, in one microstep.
    assert_eq!(step.report.microsteps, 1);
    assert_eq!(step.report.transitions, 1);
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Standing",
            "exit movement:Standing",
            "entry movement:Jumping",
            "handler weapon:Ready",
            "handler anim:Idle",
            "exit anim:Idle",
            "entry anim:Jump",
        ]
    );
    assert_eq!(timer_of(&harness, Character::JUMPING).epoch, 1);
}

#[test]
fn every_region_sees_an_event_another_region_consumed() {
    let mut harness = started();
    harness.send(Event::Move);
    // `movement` consumed `Move`; `weapon` and `anim` still ran (spec 5.6:
    // only a transition out of the parallel state short-circuits).
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Standing",
            "exit movement:Standing",
            "entry movement:Walking",
            "handler weapon:Ready",
            "handler anim:Idle",
            "exit anim:Idle",
            "entry anim:Run",
        ]
    );
}

#[test]
fn anim_sees_the_landing_that_movement_applied_in_the_same_microstep() {
    let mut harness = started();
    harness.send(Event::Jump);
    take_trace(&mut harness);

    let step = harness.advance(JUMP_TIME);
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Idle)]);
    // With a snapshot taken only at the start of the microstep (errata 6)
    // `anim` would still see `Jumping` and stay in `Jump`.
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Jumping",
            "exit movement:Jumping",
            "entry movement:Standing",
            "handler weapon:Ready",
            "handler anim:Jump",
            "exit anim:Jump",
            "entry anim:Idle",
        ]
    );
    assert_eq!(
        path(&harness),
        "Alive/movement:Standing,weapon:Ready,anim:Idle"
    );
}

#[test]
fn anim_sees_the_shot_the_weapon_fired_in_the_same_microstep() {
    let mut harness = started();
    harness.send(Event::Fire);
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Standing",
            "handler weapon:Ready",
            "exit weapon:Ready",
            "entry weapon:Firing",
            "handler anim:Idle",
            "exit anim:Idle",
            "entry anim:Shoot",
        ]
    );
}

#[test]
fn anim_does_not_shoot_when_the_weapon_refused_the_trigger() {
    let mut harness = started();
    harness.send(Event::Fire);
    take_trace(&mut harness);

    // The trigger is locked: `weapon` bubbles, and `anim`, which asks what the
    // weapon is doing, is already in the right pose and bubbles too.
    let report = harness.send(Event::Fire).report;
    assert!(!report.handled);
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Standing",
            "handler weapon:Firing",
            "handler anim:Shoot",
            "handler Alive",
        ]
    );

    harness.advance(FIRE_COOLDOWN);
    harness.send(Event::ReloadRequest);
    take_trace(&mut harness);
    harness.send(Event::Fire);
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Standing",
            "handler weapon:Reloading",
            "handler anim:Idle",
            "handler Alive",
        ],
        "no shot while reloading, so no Shoot pose"
    );
    assert_eq!(alive(&harness).anim, Anim::Idle);
}

#[test]
fn shooting_wins_over_jumping_and_the_jump_pose_returns_after_the_cooldown() {
    let mut harness = started();
    harness.send(Event::Jump);
    harness.send(Event::Fire);
    assert_eq!(alive(&harness).anim, Anim::Shoot);

    let step = harness.advance(FIRE_COOLDOWN);
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Jump)]);
    assert_eq!(alive(&harness).movement, Movement::Jumping);

    harness.advance(JUMP_TIME);
    assert_eq!(alive(&harness).anim, Anim::Idle);
}

#[test]
fn the_run_pose_returns_after_a_shot_while_walking() {
    let mut harness = started();
    harness.send(Event::Move);
    fire_once(&mut harness);
    assert_eq!(alive(&harness).anim, Anim::Run);
    harness.send(Event::Stop);
    assert_eq!(alive(&harness).anim, Anim::Idle);
}

#[test]
fn damage_falls_through_every_region_to_the_handler_of_alive() {
    let mut harness = started();
    let report = harness.send(Event::Damage(10)).report;
    assert!(report.handled);
    assert_eq!(
        take_trace(&mut harness),
        [
            "handler movement:Standing",
            "handler weapon:Ready",
            "handler anim:Idle",
            "handler Alive",
        ]
    );
}

#[test]
fn timers_of_different_regions_run_side_by_side() {
    let mut harness = started();
    fire_once(&mut harness);
    harness.send(Event::ReloadRequest);
    harness.send(Event::Jump);
    assert_eq!(timer_of(&harness, Character::RELOADING).epoch, 1);
    assert_eq!(timer_of(&harness, Character::JUMPING).epoch, 1);

    harness.advance(JUMP_TIME);
    assert_eq!(alive(&harness).movement, Movement::Standing);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
    harness.advance(RELOAD_TIME - JUMP_TIME - 1.millis());
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
    harness.advance(1.millis());
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    harness.assert_no_pending_timers();
}
