//! Timers and invokes on a machine written with `#[machine]` (spec 5.3 steps
//! 2, 8 and 9, spec 7.1, 7.2): epochs, scheduling on entry, cancelling on
//! exit, stale keys, and events built when a timer fires. The cases mirror
//! `estado-core/tests/timers.rs`, which runs them on a hand-written machine.
//! The machine is in `support/net.rs`.
#![allow(dead_code, missing_docs)]

#[path = "support/net.rs"]
mod net_machine;

use std::time::Duration;

use estado_core::{Cx, Effect, FnSink, IgnoreReason, Machine, Next, Spec};
use estado_macros::machine;
use net_machine::*;

const THREE: Duration = Duration::from_secs(3);

#[test]
fn entering_a_state_starts_its_timer_and_invoke_in_one_epoch() {
    let mut machine = started();

    let effects = send(&mut machine, Event::Connect);

    assert_eq!(
        effects,
        [
            Effect::StartTimer {
                key: timer(Net::CONNECTING_ID, 1, 0),
                after: THREE
            },
            Effect::Invoke {
                key: invoke(Net::CONNECTING_ID, 1, 0),
                task: Command::Dial(0)
            },
        ]
    );
}

#[test]
fn leaving_a_state_cancels_its_timer_and_invoke_before_the_next_entries() {
    let mut machine = started();
    send(&mut machine, Event::Connect);

    let effects = send(&mut machine, Event::Hang);

    assert_eq!(
        effects,
        [
            Effect::CancelTimer(timer(Net::CONNECTING_ID, 1, 0)),
            Effect::Cancel(invoke(Net::CONNECTING_ID, 1, 0)),
        ]
    );
}

#[test]
fn the_duration_is_computed_from_the_data_of_the_state_on_entry() {
    let mut machine = started();
    send(&mut machine, Event::Connect);
    let effects = fire(&mut machine, timer(Net::CONNECTING_ID, 1, 0)).0;

    assert_eq!(
        effects,
        [
            Effect::CancelTimer(timer(Net::CONNECTING_ID, 1, 0)),
            Effect::Cancel(invoke(Net::CONNECTING_ID, 1, 0)),
            Effect::StartTimer {
                key: timer(Net::BACKOFF_ID, 1, 0),
                after: Duration::from_millis(200)
            },
        ]
    );
    assert_eq!(*machine.state(), Net::Backoff { attempt: 1 });

    // The next failure waits twice as long.
    fire(&mut machine, timer(Net::BACKOFF_ID, 1, 0));
    let effects = fire(&mut machine, timer(Net::CONNECTING_ID, 2, 0)).0;
    assert!(effects.contains(&Effect::StartTimer {
        key: timer(Net::BACKOFF_ID, 2, 0),
        after: Duration::from_millis(400)
    }));
}

#[test]
fn every_entry_gets_a_new_epoch_and_old_keys_are_stale() {
    let mut machine = started();
    send(&mut machine, Event::Connect);
    let first = timer(Net::CONNECTING_ID, 1, 0);
    fire(&mut machine, first);
    fire(&mut machine, timer(Net::BACKOFF_ID, 1, 0));
    assert_eq!(*machine.state(), Net::Connecting { attempt: 1 });

    // `Connecting` was entered again: the key of its first instance is stale
    // even though the state is active.
    let (effects, ignored) = fire(&mut machine, first);
    assert!(effects.is_empty());
    assert_eq!(ignored, Some(IgnoreReason::Stale));
    assert_eq!(*machine.state(), Net::Connecting { attempt: 1 });

    let (_, ignored) = fire(&mut machine, timer(Net::CONNECTING_ID, 2, 0));
    assert_eq!(ignored, None);
    assert_eq!(*machine.state(), Net::Backoff { attempt: 2 });
}

#[test]
fn a_key_of_a_state_that_is_not_active_is_stale() {
    let mut machine = started();
    let (_, ignored) = fire(&mut machine, timer(Net::CONNECTING_ID, 1, 0));
    assert_eq!(ignored, Some(IgnoreReason::Stale));
    assert_eq!(*machine.state(), Net::Idle);
}

#[test]
fn an_invoke_result_is_accepted_only_for_the_active_instance() {
    let mut machine = started();
    send(&mut machine, Event::Connect);

    let (_, ignored) = complete(
        &mut machine,
        invoke(Net::CONNECTING_ID, 7, 0),
        Event::Connected(1),
    );
    assert_eq!(ignored, Some(IgnoreReason::Stale));
    assert_eq!(*machine.state(), Net::Connecting { attempt: 0 });

    let (effects, ignored) = complete(
        &mut machine,
        invoke(Net::CONNECTING_ID, 1, 0),
        Event::Connected(5),
    );
    assert_eq!(ignored, None);
    assert_eq!(*machine.state(), Net::Online(Online { count: 5 }));
    // The result left `Connecting`: its tasks are cancelled, `Online` starts.
    assert_eq!(
        effects,
        [
            Effect::CancelTimer(timer(Net::CONNECTING_ID, 1, 0)),
            Effect::Cancel(invoke(Net::CONNECTING_ID, 1, 0)),
            Effect::StartTimer {
                key: timer(Net::ONLINE_ID, 1, 0),
                after: Duration::from_secs(10)
            },
            Effect::Invoke {
                key: invoke(Net::ONLINE_ID, 1, 0),
                task: Command::Probe(5)
            },
        ]
    );
}

#[test]
fn the_event_of_a_timer_is_built_from_the_data_when_it_fires() {
    let mut machine = started();
    send(&mut machine, Event::Connect);
    complete(
        &mut machine,
        invoke(Net::CONNECTING_ID, 1, 0),
        Event::Connected(5),
    );
    send(&mut machine, Event::Add(4));

    // `Expired(online.count)`: the data changed after the timer started, and
    // the handler echoes what it received.
    let (_, ignored) = fire(&mut machine, timer(Net::ONLINE_ID, 1, 0));
    assert_eq!(ignored, None);
    assert_eq!(*machine.state(), Net::Online(Online { count: 1009 }));
}

#[test]
fn a_state_may_have_several_timers_told_apart_by_index() {
    let mut machine = started();
    let effects = send(&mut machine, Event::SlowTick);
    assert_eq!(
        effects,
        [
            Effect::StartTimer {
                key: timer(Net::TICKING_ID, 1, 0),
                after: Duration::from_secs(1)
            },
            Effect::StartTimer {
                key: timer(Net::TICKING_ID, 1, 1),
                after: Duration::from_millis(250)
            },
        ]
    );
    // Each key builds its own event: index 1 is the fast tick, which leaves.
    let (effects, ignored) = fire(&mut machine, timer(Net::TICKING_ID, 1, 1));
    assert_eq!(ignored, None);
    assert_eq!(*machine.state(), Net::Idle);
    assert_eq!(
        effects,
        [
            Effect::CancelTimer(timer(Net::TICKING_ID, 1, 0)),
            Effect::CancelTimer(timer(Net::TICKING_ID, 1, 1)),
        ]
    );
    // There is no timer 2.
    send(&mut machine, Event::SlowTick);
    let (_, ignored) = fire(&mut machine, timer(Net::TICKING_ID, 2, 2));
    assert_eq!(ignored, Some(IgnoreReason::Stale));
}

#[test]
fn a_compound_state_and_its_substate_have_timers_of_their_own() {
    let mut machine = started();
    let effects = send(&mut machine, Event::Hang);
    // `Idle` answers `Hang` by entering the compound `Session`, whose
    // initial substate `Active` has no timer.
    assert_eq!(
        effects,
        [Effect::StartTimer {
            key: timer(Net::SESSION_ID, 1, 0),
            after: Duration::from_secs(30)
        }]
    );

    // Pausing starts the timer of the substate; the compound's keeps running.
    let effects = send(&mut machine, Event::Pause);
    assert_eq!(
        effects,
        [Effect::StartTimer {
            key: timer(Net::PAUSED_ID, 1, 0),
            after: Duration::from_secs(60)
        }]
    );

    // Leaving the compound cancels the substate first, then the compound.
    let effects = send(&mut machine, Event::Hang);
    assert_eq!(
        effects,
        [
            Effect::CancelTimer(timer(Net::PAUSED_ID, 1, 0)),
            Effect::CancelTimer(timer(Net::SESSION_ID, 1, 0)),
        ]
    );
}

#[test]
fn the_graph_counts_the_timers_and_invokes_of_each_state() {
    let graph = <Net as Spec>::GRAPH;
    assert_eq!(graph.state(Net::CONNECTING_ID).timers, 1);
    assert_eq!(graph.state(Net::CONNECTING_ID).invokes, 1);
    assert_eq!(graph.state(Net::TICKING_ID).timers, 2);
    assert_eq!(graph.state(Net::IDLE_ID).timers, 0);
}

#[machine(event = Event, effect = Command)]
mod clock {
    use super::*;

    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Clock {
        /// The initial state has a timer: the initial entry starts it.
        #[initial]
        #[after(Duration::from_secs(1) => Event::SlowTick)]
        Running,
        Stopped,
    }

    #[handlers]
    impl Clock {
        #[on(Running)]
        fn on_running(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::SlowTick => goto!(Stopped),
                _ => bubble!(),
            }
        }

        #[on(Stopped)]
        fn on_stopped(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Hang => goto!(Running),
                _ => bubble!(),
            }
        }
    }
}

#[test]
fn the_initial_entry_starts_the_timers_of_the_initial_state() {
    let mut machine = Machine::<clock::Clock>::new(());
    let mut effects = Vec::new();
    machine
        .start_into(&mut FnSink(|effect| effects.push(effect)))
        .unwrap();
    assert_eq!(
        effects,
        [Effect::StartTimer {
            key: timer(clock::Clock::RUNNING_ID, 1, 0),
            after: Duration::from_secs(1)
        }]
    );
}
