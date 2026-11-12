//! After a panic in any phase, every option leaves the machine poisoned,
//! `send` returns `Poisoned`, and no value leaks or is dropped twice.

use estado_spike_poison::fixtures_flat::*;
use estado_spike_poison::fixtures_full256::Full256;
use estado_spike_poison::fixtures_marked::Marked;
use estado_spike_poison::fixtures_player::{Active, ActiveSub, Player};
use estado_spike_poison::fixtures_tracked::{Tracked, live_probes};
use estado_spike_poison::flag_machine::FlagMachine;
use estado_spike_poison::mirror_cast_machine::MirrorCastMachine;
use estado_spike_poison::mirror_machine::MirrorMachine;
use estado_spike_poison::option_machine::OptionMachine;
use estado_spike_poison::slot_machine::SlotMachine;
use estado_spike_poison::uninit_machine::UninitMachine;
use estado_spike_poison::{MachineApi, Script, SendError, Spec, StateAccess};
use std::fmt::Debug;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;

const PANICKING_SCRIPTS: [Script; 4] = [
    Script::PanicInExit,
    Script::PanicInConstructor,
    Script::PanicInNestedConstructor,
    Script::PanicInEntry,
];

fn silence_panic_output() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| std::panic::set_hook(Box::new(|_| {})));
}

/// Needs only the safe part of the API (all options).
fn check_poisoning<M: Spec, K: MachineApi<M, ()>>() {
    silence_panic_output();
    for script in PANICKING_SCRIPTS {
        let mut machine = K::create(M::initial(), ());
        machine
            .send(Script::Plain)
            .expect("first transition succeeds");
        assert!(!machine.is_poisoned());
        let outcome = catch_unwind(AssertUnwindSafe(|| machine.send(script)));
        assert!(outcome.is_err(), "{script:?} must panic");
        assert!(machine.is_poisoned(), "{script:?} must poison");
        assert_eq!(machine.send(Script::Plain), Err(SendError::Poisoned));
        assert_eq!(machine.send(Script::Nested), Err(SendError::Poisoned));
        assert!(machine.is_poisoned());
    }
}

/// Needs `state()` and `try_state()`.
fn check_views<M: Spec + Debug + PartialEq, K: MachineApi<M, ()> + StateAccess<M>>() {
    silence_panic_output();
    let mut machine = K::create(M::initial(), ());
    assert_eq!(machine.state(), &M::initial());
    assert_eq!(machine.try_state(), Some(&M::initial()));
    machine.send(Script::Plain).unwrap();
    assert_eq!(machine.state(), &M::initial().next());

    for script in PANICKING_SCRIPTS {
        let mut machine = K::create(M::initial(), ());
        let _ = catch_unwind(AssertUnwindSafe(|| machine.send(script)));
        // `Nested` panic scripts on a non-Active root fall back to `next`,
        // which does not panic; only assert when the machine did poison.
        if machine.is_poisoned() {
            assert!(machine.try_state().is_none());
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                machine.state();
            }));
            assert!(outcome.is_err(), "state() must panic when poisoned");
        }
    }
}

/// No leak and no double drop in any scenario (probes created == dropped).
fn check_no_leak<K: MachineApi<Tracked, ()>>() {
    silence_panic_output();
    for script in PANICKING_SCRIPTS
        .into_iter()
        .chain([Script::Plain, Script::Nested])
    {
        {
            let mut machine = K::create(Tracked::initial(), ());
            let _ = catch_unwind(AssertUnwindSafe(|| machine.send(Script::Plain)));
            let _ = catch_unwind(AssertUnwindSafe(|| machine.send(script)));
            let _ = machine.send(Script::Plain);
        }
        assert_eq!(live_probes(), 0, "leak or double drop after {script:?}");
    }
}

macro_rules! poisoning_suite {
    ($module:ident, $machine:ident) => {
        mod $module {
            use super::*;
            #[test]
            fn unit() {
                check_poisoning::<Unit, $machine<Unit, ()>>();
            }
            #[test]
            fn boxed() {
                check_poisoning::<Boxed, $machine<Boxed, ()>>();
            }
            #[test]
            fn owned() {
                check_poisoning::<Owned, $machine<Owned, ()>>();
            }
            #[test]
            fn only() {
                check_poisoning::<Only, $machine<Only, ()>>();
            }
        }
    };
}

macro_rules! full_suite {
    ($module:ident, $machine:ident) => {
        mod $module {
            use super::*;
            #[test]
            fn unit() {
                check_poisoning::<Unit, $machine<Unit, ()>>();
                check_views::<Unit, $machine<Unit, ()>>();
            }
            #[test]
            fn boxed() {
                check_poisoning::<Boxed, $machine<Boxed, ()>>();
                check_views::<Boxed, $machine<Boxed, ()>>();
            }
            #[test]
            fn strs() {
                check_poisoning::<Strs, $machine<Strs, ()>>();
                check_views::<Strs, $machine<Strs, ()>>();
            }
            #[test]
            fn owned() {
                check_poisoning::<Owned, $machine<Owned, ()>>();
                check_views::<Owned, $machine<Owned, ()>>();
            }
            #[test]
            fn pair() {
                check_poisoning::<Pair, $machine<Pair, ()>>();
                check_views::<Pair, $machine<Pair, ()>>();
            }
            #[test]
            fn only() {
                check_poisoning::<Only, $machine<Only, ()>>();
                check_views::<Only, $machine<Only, ()>>();
            }
            #[test]
            fn full256() {
                check_poisoning::<Full256, $machine<Full256, ()>>();
                check_views::<Full256, $machine<Full256, ()>>();
            }
            #[test]
            fn player() {
                check_poisoning::<Player, $machine<Player, ()>>();
                check_views::<Player, $machine<Player, ()>>();
            }
            #[test]
            fn tracked() {
                check_poisoning::<Tracked, $machine<Tracked, ()>>();
                check_no_leak::<$machine<Tracked, ()>>();
            }
        }
    };
}

full_suite!(option_machine, OptionMachine);
full_suite!(slot_machine, SlotMachine);
full_suite!(flag_machine, FlagMachine);
full_suite!(uninit_machine, UninitMachine);
poisoning_suite!(mirror_machine, MirrorMachine);

mod mirror_cast_machine {
    use super::*;
    #[test]
    fn marked() {
        check_poisoning::<Marked, MirrorCastMachine<Marked, ()>>();
        check_views::<Marked, MirrorCastMachine<Marked, ()>>();
    }
}

#[test]
fn nested_transition_rebuilds_the_path_and_keeps_superstate_data() {
    fn run<K: MachineApi<Player, ()> + StateAccess<Player>>() {
        let mut machine = K::create(Player::initial(), ());
        machine.send(Script::Plain).unwrap();
        machine.send(Script::Nested).unwrap();
        let expected = Player::Active(Active {
            track: 1,
            buf: vec![0; 4],
            sub: ActiveSub::Paused { pos: 0 },
        });
        assert_eq!(machine.state(), &expected);
    }
    run::<OptionMachine<Player, ()>>();
    run::<FlagMachine<Player, ()>>();
    run::<UninitMachine<Player, ()>>();
}

#[test]
fn match_sees_only_the_user_variants() {
    let machine = OptionMachine::<Player, ()>::create(Player::initial(), ());
    // Exhaustive match, no wildcard and no hidden variant.
    let label = match machine.state() {
        Player::Idle => "idle",
        Player::Active(_) => "active",
        Player::Failed { .. } => "failed",
    };
    assert_eq!(label, "idle");
}

#[test]
fn slot_machine_starts_implicitly_on_first_send() {
    let mut machine = SlotMachine::<Player, ()>::unstarted(());
    assert!(!machine.is_started());
    assert!(!machine.is_poisoned());
    assert!(machine.try_state().is_none());
    machine.send(Script::Plain).unwrap();
    assert!(machine.is_started());
    assert_eq!(machine.state(), &Player::initial().next());
}

#[test]
fn slot_machine_panic_in_implicit_start_poisons() {
    silence_panic_output();
    let mut machine = SlotMachine::<Tracked, ()>::unstarted(());
    // The initial entry runs with `Script::Plain`, so make the transition fail instead.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        machine.send(Script::PanicInConstructor)
    }));
    assert!(machine.is_poisoned());
    assert_eq!(machine.send(Script::Plain), Err(SendError::Poisoned));
    drop(machine);
    assert_eq!(live_probes(), 0);
}
