//! AC-01 (exit/entry order of every row of spec table 5.2) and AC-09
//! (shallow and deep history) on the hand-written `Tree` machine.

mod support;

use estado_core::Machine;
use support::tree::model::{Active, ActiveSub, Event, Playing, PlayingSub, Tree};

fn idle() -> Machine<Tree> {
    let mut machine = Machine::<Tree>::new(Vec::new());
    machine.start().unwrap();
    machine.ctx_mut().clear();
    machine
}

/// In `Active/Playing/Normal` with an empty log.
fn normal() -> Machine<Tree> {
    let mut machine = idle();
    machine.send(Event::ToActive).unwrap();
    machine.ctx_mut().clear();
    machine
}

fn send(machine: &mut Machine<Tree>, event: Event) -> Vec<String> {
    machine.send(event).unwrap();
    std::mem::take(machine.ctx_mut())
}

fn playing(count: u32, speed: u8, sub: PlayingSub) -> Tree {
    Tree::Active(Active {
        count,
        sub: ActiveSub::Playing(Playing { speed, sub }),
    })
}

#[test]
fn ac_01_entering_a_compound_enters_its_initials() {
    let mut machine = idle();
    assert_eq!(
        send(&mut machine, Event::ToActive),
        [
            "handler outside",
            "exit Idle",
            "entry Active",
            "entry Playing",
            "entry Normal"
        ]
    );
}

#[test]
fn ac_01_target_outside_the_handler_subtree_exits_up_to_the_lca() {
    let mut machine = normal();
    assert_eq!(
        send(&mut machine, Event::PlayingToIdle),
        [
            "handler Normal",
            "handler Playing",
            "exit Normal",
            "exit Playing",
            "exit Active",
            "entry Idle"
        ]
    );
    assert_eq!(*machine.state(), Tree::Idle);
}

#[test]
fn ac_01_cousin_transition_stops_below_the_common_ancestor() {
    let mut machine = normal();
    assert_eq!(
        send(&mut machine, Event::ToPaused),
        [
            "handler Normal",
            "exit Normal",
            "exit Playing",
            "entry Paused"
        ]
    );
}

#[test]
fn ac_01_local_transition_to_a_descendant_of_the_handler() {
    let mut machine = normal();
    assert_eq!(
        send(&mut machine, Event::ActiveToFast),
        [
            "handler Normal",
            "handler Playing",
            "handler Active",
            "exit Normal",
            "exit Playing",
            "entry Playing",
            "entry Fast"
        ]
    );
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Fast));
}

#[test]
fn ac_01_local_transition_to_an_ancestor_keeps_the_ancestor() {
    let mut machine = normal();
    machine.send(Event::StayInActive).unwrap();
    machine.ctx_mut().clear();
    assert_eq!(
        send(&mut machine, Event::NormalToActive),
        [
            "handler Normal",
            "exit Normal",
            "exit Playing",
            "entry Playing",
            "entry Normal"
        ]
    );
    assert_eq!(*machine.state(), playing(1, 1, PlayingSub::Normal));
}

#[test]
fn ac_01_external_transition_to_an_ancestor_exits_and_enters_it() {
    let mut machine = normal();
    assert_eq!(
        send(&mut machine, Event::NormalToActiveExternal),
        [
            "handler Normal",
            "exit Normal",
            "exit Playing",
            "exit Active",
            "entry Active",
            "entry Playing",
            "entry Normal"
        ]
    );
    assert_eq!(*machine.state(), playing(100, 1, PlayingSub::Normal));
}

#[test]
fn ac_01_goto_the_handler_state_exits_and_enters_it() {
    let mut machine = normal();
    let report = machine.send(Event::NormalToItself).unwrap().report;
    assert_eq!(report.transitions, 1);
    assert_eq!(
        std::mem::take(machine.ctx_mut()),
        ["handler Normal", "exit Normal", "entry Normal"]
    );
}

#[test]
fn ac_01_reenter_restarts_substates_and_keeps_own_data() {
    let mut machine = normal();
    machine.send(Event::SetSpeed(4)).unwrap();
    machine.send(Event::NormalToFast).unwrap();
    machine.ctx_mut().clear();
    assert_eq!(
        send(&mut machine, Event::ReenterPlaying),
        [
            "handler Playing",
            "exit Fast",
            "exit Playing",
            "entry Playing",
            "entry Normal"
        ]
    );
    assert_eq!(*machine.state(), playing(0, 4, PlayingSub::Normal));
}

#[test]
fn ac_01_stay_runs_no_exit_or_entry() {
    let mut machine = normal();
    let report = machine.send(Event::StayInActive).unwrap().report;
    assert_eq!(report.transitions, 0);
    assert_eq!(
        std::mem::take(machine.ctx_mut()),
        ["handler Normal", "handler Playing", "handler Active"]
    );
    assert_eq!(*machine.state(), playing(1, 1, PlayingSub::Normal));
}

#[test]
fn ac_01_constructor_runs_after_exits_and_before_entries() {
    let mut machine = normal();
    machine.send(Event::StayInActive).unwrap();
    machine.send(Event::StayInActive).unwrap();
    machine.send(Event::ToPaused).unwrap();
    machine.ctx_mut().clear();
    assert_eq!(
        send(&mut machine, Event::ActiveToStopped),
        [
            "handler Paused",
            "handler Active",
            "exit Paused",
            "exit Active",
            "construct Stopped",
            "entry Stopped"
        ]
    );
    assert_eq!(*machine.state(), Tree::Stopped { count: 2 });
}

#[test]
fn ac_09_deep_history_without_a_record_uses_the_initial() {
    let mut machine = idle();
    machine.send(Event::ToActiveWithHistory).unwrap();
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Normal));
}

#[test]
fn ac_09_deep_history_restores_the_full_substate_value() {
    let mut machine = normal();
    machine.send(Event::SetSpeed(3)).unwrap();
    machine.send(Event::NormalToFast).unwrap();
    machine.send(Event::PlayingToIdle).unwrap();
    machine.ctx_mut().clear();
    assert_eq!(
        send(&mut machine, Event::ToActiveWithHistory),
        [
            "handler outside",
            "exit Idle",
            "entry Active",
            "entry Playing",
            "entry Fast"
        ]
    );
    assert_eq!(*machine.state(), playing(0, 3, PlayingSub::Fast));
}

#[test]
fn ac_09_deep_history_is_recorded_when_a_constructor_takes_the_rest() {
    let mut machine = normal();
    machine.send(Event::NormalToFast).unwrap();
    machine.send(Event::ActiveToStopped).unwrap();
    machine.send(Event::ToActiveWithHistory).unwrap();
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Fast));
}

#[test]
fn ac_09_shallow_history_restores_the_substate_and_rebuilds_the_data() {
    let mut machine = normal();
    machine.send(Event::SetSpeed(5)).unwrap();
    machine.send(Event::NormalToFast).unwrap();
    machine.send(Event::ToPaused).unwrap();
    machine.ctx_mut().clear();
    assert_eq!(
        send(&mut machine, Event::PausedToPlayingWithHistory),
        [
            "handler Paused",
            "exit Paused",
            "entry Playing",
            "entry Fast"
        ]
    );
    // `#[history_init]` rebuilds the data: speed back to 1.
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Fast));
}

#[test]
fn ac_09_plain_goto_ignores_recorded_history() {
    let mut machine = normal();
    machine.send(Event::NormalToFast).unwrap();
    machine.send(Event::PlayingToIdle).unwrap();
    machine.send(Event::ToActive).unwrap();
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Normal));
}

#[test]
fn path_and_is_in_follow_the_active_configuration() {
    let mut machine = normal();
    assert_eq!(machine.path().to_string(), "Active/Playing/Normal");
    assert!(machine.is_in(support::tree::model::PLAYING));
    machine.send(Event::ToPaused).unwrap();
    assert_eq!(machine.path().to_string(), "Active/Paused");
    assert!(!machine.is_in(support::tree::model::PLAYING));
}
