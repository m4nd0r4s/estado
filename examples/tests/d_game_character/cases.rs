// The test cases of scenario D, shared by the hand-written version and the
// `#[machine]` version: the including module provides `scenario` (the module
// under test).
mod airborne {
    include!("airborne.rs");
}
mod fuzz {
    include!("fuzz.rs");
}
mod health {
    include!("health.rs");
}
mod hot_path {
    include!("hot_path.rs");
}
mod preemption {
    include!("preemption.rs");
}
mod regions {
    include!("regions.rs");
}
#[allow(dead_code)]
mod support {
    include!("support.rs");
}

use estado_core::IgnoreReason;
use scenario::{
    Anim, Character, Command, Event, FIRE_COOLDOWN, JUMP_TIME, MAGAZINE_SIZE, Movement, Sound,
    Weapon,
};
use estado_test::DurationExt;

use support::{alive, empty_the_magazine, fire_once, path, started, take_trace, timer_of};

#[test]
fn start_enters_alive_and_the_initial_state_of_every_region() {
    let mut harness = estado_test::Harness::new(estado_core::Machine::<Character>::new(
        scenario::GameCtx::traced(),
    ));
    let step = harness.start();
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Idle)]);
    assert_eq!(
        take_trace(&mut harness),
        [
            "entry Alive",
            "entry movement:Standing",
            "entry weapon:Ready",
            "entry anim:Idle",
        ]
    );
    assert_eq!(
        path(&harness),
        "Alive/movement:Standing,weapon:Ready,anim:Idle"
    );
    assert_eq!(alive(&harness).health, 100);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE);
    harness.assert_no_pending_timers();
}

#[test]
fn walking_starts_and_stops_with_the_matching_animation() {
    let mut harness = started();
    let step = harness.send(Event::Move);
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Run)]);
    assert_eq!(alive(&harness).movement, Movement::Walking);
    assert_eq!(alive(&harness).anim, Anim::Run);

    // Walking again is handled (consumed) and changes nothing.
    let report = harness.send(Event::Move).report;
    assert!(report.handled);
    assert_eq!(report.transitions, 0);

    let step = harness.send(Event::Stop);
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Idle)]);
    assert_eq!(alive(&harness).movement, Movement::Standing);
}

#[test]
fn crouching_and_standing_up_do_not_change_the_pose() {
    let mut harness = started();
    let step = harness.send(Event::Crouch);
    step.expect_no_effect();
    assert_eq!(alive(&harness).movement, Movement::Crouching);
    harness.send(Event::Jump);
    assert_eq!(
        alive(&harness).movement,
        Movement::Crouching,
        "no jumping out of a crouch"
    );
    harness.send(Event::Stand);
    assert_eq!(alive(&harness).movement, Movement::Standing);
    assert_eq!(alive(&harness).anim, Anim::Idle);
}

#[test]
fn firing_spawns_a_projectile_locks_the_trigger_and_starts_the_cooldown() {
    let mut harness = started();
    let step = harness.send(Event::Fire);
    assert_eq!(
        step.effects,
        [
            Command::SpawnProjectile,
            Command::PlaySound(Sound::Gunshot),
            Command::PlayAnimation(Anim::Shoot),
        ]
    );
    assert_eq!(alive(&harness).weapon, Weapon::Firing);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE - 1);
    assert_eq!(
        harness.pending_timers()[0].deadline,
        FIRE_COOLDOWN,
        "the cooldown starts with the shot"
    );

    // A second pull of the trigger during the cooldown is ignored.
    let report = harness.send(Event::Fire).report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE - 1);

    let step = harness.advance(FIRE_COOLDOWN);
    assert_eq!(step.effects, [Command::PlayAnimation(Anim::Idle)]);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    harness.assert_no_pending_timers();
}

#[test]
fn the_last_round_leaves_the_weapon_empty_and_the_trigger_clicks() {
    let mut harness = started();
    empty_the_magazine(&mut harness);
    assert_eq!(alive(&harness).weapon, Weapon::Empty);
    assert_eq!(alive(&harness).ammo, 0);

    let step = harness.send(Event::Fire);
    assert_eq!(step.effects, [Command::PlaySound(Sound::EmptyClick)]);
    assert!(step.report.handled, "the click is a handled event");
    assert_eq!(step.report.transitions, 0);
    assert_eq!(alive(&harness).weapon, Weapon::Empty);
}

#[test]
fn reloading_takes_the_reload_time_and_refills_the_magazine() {
    let mut harness = started();
    fire_once(&mut harness);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE - 1);

    let step = harness.send(Event::ReloadRequest);
    assert_eq!(step.effects, [Command::PlaySound(Sound::ReloadStart)]);
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
    let reload = timer_of(&harness, Character::RELOADING);
    assert_eq!(reload.epoch, 1);

    // The trigger is locked, a second request is absorbed.
    assert_eq!(
        harness.send(Event::Fire).report.ignored,
        Some(IgnoreReason::Unhandled)
    );
    assert_eq!(harness.send(Event::ReloadRequest).report.transitions, 0);

    harness.advance(1499.millis());
    assert_eq!(alive(&harness).weapon, Weapon::Reloading);
    harness.advance(1.millis());
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    assert_eq!(alive(&harness).ammo, MAGAZINE_SIZE);
    harness.assert_no_pending_timers();
}

#[test]
fn a_full_magazine_ignores_the_reload_request_without_deferring_it() {
    let mut harness = started();
    let report = harness.send(Event::ReloadRequest).report;
    assert!(report.handled);
    assert_eq!(report.transitions, 0);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
    harness.assert_no_pending_timers();

    // Had it been deferred, the next configuration change would redeliver
    // it; jumping must not start a reload of a full magazine.
    harness.send(Event::Jump);
    harness.advance(JUMP_TIME);
    assert_eq!(alive(&harness).weapon, Weapon::Ready);
}

#[test]
fn events_nobody_handles_are_ignored() {
    let mut harness = started();
    for event in [
        Event::Land,
        Event::ReloadDone,
        Event::CooldownElapsed,
        Event::Respawn,
    ] {
        let report = harness.send(event).report;
        assert_eq!(report.ignored, Some(IgnoreReason::Unhandled), "{event:?}");
        assert!(!report.handled, "{event:?}");
    }
    assert_eq!(
        path(&harness),
        "Alive/movement:Standing,weapon:Ready,anim:Idle"
    );
}

#[test]
fn the_state_is_small_enough_to_keep_by_the_thousand() {
    use estado_core::{Machine, Slot};
    use scenario::GameCtx;

    assert_eq!(size_of::<Event>(), 4);
    assert_eq!(size_of::<GameCtx>(), size_of::<usize>());
    assert!(size_of::<Character>() <= 8, "{}", size_of::<Character>());
    // AC-15: the started and poisoned markers fit in the niches of the root.
    assert_eq!(size_of::<Slot<Character>>(), size_of::<Character>());
    println!(
        "size_of::<Machine<Character>>() = {}",
        size_of::<Machine<Character>>()
    );
}
