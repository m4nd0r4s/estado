// A long, deterministic pseudo-random run (a fixed LCG) with the invariants
// of the machine checked after every operation, and the determinism of spec
// 1.2: the same events give the same states and effects.

use std::collections::BTreeSet;
use std::time::Duration;

use estado_core::StateId;
use super::scenario::{
    Alive, Anim, Character, Command, MAGAZINE_SIZE, MAX_HEALTH, Movement, Sound, Weapon,
};
use estado_test::Harness;

use super::support::{Lcg, random_event, started};

/// Operations per run.
const OPERATIONS: usize = 6_000;

/// The pose `anim` must have, derived from the other two regions.
fn expected_pose(alive: &Alive) -> Anim {
    if alive.weapon == Weapon::Firing {
        Anim::Shoot
    } else if alive.movement == Movement::Jumping {
        Anim::Jump
    } else if alive.movement == Movement::Walking {
        Anim::Run
    } else {
        Anim::Idle
    }
}

/// The states whose instances own a timer right now.
fn states_with_a_timer(alive: &Alive) -> BTreeSet<StateId> {
    let mut states = BTreeSet::new();
    if alive.movement == Movement::Jumping {
        states.insert(Character::JUMPING);
    }
    match alive.weapon {
        Weapon::Firing => states.insert(Character::FIRING),
        Weapon::Reloading => states.insert(Character::RELOADING),
        Weapon::Ready | Weapon::Empty => false,
    };
    states
}

#[track_caller]
fn check_invariants(harness: &Harness<Character>, context: &str) {
    assert!(!harness.machine().is_poisoned(), "poisoned after {context}");
    let pending: Vec<StateId> = harness
        .pending_timers()
        .iter()
        .map(|timer| timer.key.state)
        .collect();
    assert!(
        harness.pending_invokes().is_empty(),
        "this machine has no invoke"
    );
    match harness.machine().state() {
        Character::Dead => {
            assert!(
                pending.is_empty(),
                "Dead with timers {pending:?} after {context}"
            );
            assert_eq!(harness.machine().path().to_string(), "Dead");
        }
        Character::Alive(alive) => {
            assert!(
                (1..=MAX_HEALTH).contains(&alive.health),
                "health {} after {context}",
                alive.health
            );
            assert!(alive.ammo <= MAGAZINE_SIZE, "ammo after {context}");
            // `Ready` never has an empty magazine; `Empty` always has.
            assert!(
                alive.weapon != Weapon::Ready || alive.ammo > 0,
                "Ready without ammo after {context}"
            );
            assert!(
                alive.weapon != Weapon::Empty || alive.ammo == 0,
                "Empty with ammo after {context}"
            );
            // The pose is always the one derived from the other regions: it
            // sees their transitions of the same microstep (ADR 0005).
            assert_eq!(alive.anim, expected_pose(alive), "pose after {context}");
            // Exactly the active timer states have a pending timer, each once.
            let mut sorted = pending.clone();
            sorted.sort();
            let expected: Vec<StateId> = states_with_a_timer(alive).into_iter().collect();
            assert_eq!(sorted, expected, "timers after {context}");
        }
    }
}

/// What one run produced, to compare runs and to check coverage.
#[derive(Default, PartialEq, Debug)]
struct Outcome {
    log: Vec<String>,
    deaths: usize,
    respawns: usize,
    shots: usize,
    reloads: usize,
    deferred_reloads_started_by_landing: usize,
}

fn run(seed: u64) -> Outcome {
    let mut harness = started();
    let mut rng = Lcg::new(seed);
    let mut outcome = Outcome::default();
    for step in 0..OPERATIONS {
        let (label, effects, report) = if rng.below(4) == 0 {
            let duration = Duration::from_millis(u64::from(rng.below(700)));
            let view = harness.advance(duration);
            (
                format!("advance {duration:?}"),
                view.effects.to_vec(),
                view.report,
            )
        } else {
            let event = random_event(&mut rng);
            let view = harness.send(event);
            (format!("{event:?}"), view.effects.to_vec(), view.report)
        };
        let context = format!("#{step} {label}");
        check_invariants(&harness, &context);
        assert!(report.microsteps <= 64, "livelock guard after {context}");

        outcome.deaths += effects
            .iter()
            .filter(|command| **command == Command::PlaySound(Sound::Death))
            .count();
        outcome.shots += effects
            .iter()
            .filter(|command| **command == Command::SpawnProjectile)
            .count();
        outcome.reloads += effects
            .iter()
            .filter(|command| **command == Command::PlaySound(Sound::ReloadStart))
            .count();
        if label == "Respawn" && harness.machine().path().to_string() != "Dead" {
            outcome.respawns += 1;
        }
        if label.starts_with("advance")
            && effects.contains(&Command::PlaySound(Sound::ReloadStart))
            && effects.contains(&Command::PlayAnimation(Anim::Idle))
        {
            outcome.deferred_reloads_started_by_landing += 1;
        }
        outcome.log.push(format!(
            "{label} -> {} {effects:?}",
            harness.machine().path()
        ));
    }
    outcome
}

#[test]
fn invariants_hold_after_every_operation_of_a_long_random_run() {
    for seed in [1, 42, 2026] {
        let outcome = run(seed);
        // The run must actually visit the interesting corners.
        assert!(
            outcome.deaths >= 20,
            "seed {seed}: deaths {}",
            outcome.deaths
        );
        assert!(
            outcome.respawns >= 10,
            "seed {seed}: respawns {}",
            outcome.respawns
        );
        assert!(outcome.shots >= 200, "seed {seed}: shots {}", outcome.shots);
        assert!(
            outcome.reloads >= 50,
            "seed {seed}: reloads {}",
            outcome.reloads
        );
        assert!(
            outcome.deferred_reloads_started_by_landing >= 1,
            "seed {seed}: no deferred reload was ever redelivered"
        );
    }
}

#[test]
fn the_same_events_give_the_same_states_and_effects() {
    let first = run(7);
    let second = run(7);
    assert_eq!(first, second);
    assert_ne!(first.log, run(8).log, "a different seed is a different run");
}
