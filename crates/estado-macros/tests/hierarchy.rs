//! AC-01 on a machine written with `#[machine]`: the exit/entry order of
//! every row of spec table 5.2, three levels deep. The cases mirror
//! `estado-core/tests/hierarchy.rs`, which runs them on the hand-written
//! `Tree`, plus AC-09 (shallow and deep history).
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    PlayingToIdle,
    ToPaused,
    NormalToFast,
    ActiveToFast,
    NormalToActive,
    NormalToActiveExternal,
    NormalToItself,
    ReenterPlaying,
    SetSpeed(u8),
    StayInActive,
    ActiveToStopped,
    ToActive,
    UnknownEvent,
    ToPausedFromAnywhere,
    ToActiveAtSpeed(u8),
    ToActiveWithHistory,
    PausedToPlayingWithHistory,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, tree::Tree, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>)]
mod tree {
    use super::*;

    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Tree {
        #[initial]
        Idle,
        Stopped {
            count: u32,
        },
        Active(Active),
    }

    #[state]
    #[history(deep, keep)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Active {
        pub count: u32,
        #[substates]
        pub sub: ActiveSub,
    }

    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ActiveSub {
        #[initial(with = initial_playing)]
        Playing(Playing),
        #[unhandled(error)]
        Paused,
    }

    #[state]
    #[history]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Playing {
        pub speed: u8,
        #[substates]
        pub sub: PlayingSub,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PlayingSub {
        #[initial]
        Normal,
        Fast,
    }

    fn initial_playing() -> ActiveSub {
        ActiveSub::Playing(Playing {
            speed: 1,
            sub: PlayingSub::Normal,
        })
    }

    #[handlers]
    impl Tree {
        #[entry(Idle)]
        fn entry_idle(cx: &mut Cx<Self>) {
            note(cx, "entry Idle");
        }
        #[exit(Idle)]
        fn exit_idle(cx: &mut Cx<Self>) {
            note(cx, "exit Idle");
        }
        #[entry(Stopped)]
        fn entry_stopped(_stopped: StoppedMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Stopped");
        }
        #[exit(Stopped)]
        fn exit_stopped(_stopped: StoppedMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Stopped");
        }
        #[entry(Active)]
        fn entry_active(_up: ActiveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Active");
        }
        #[exit(Active)]
        fn exit_active(_up: ActiveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Active");
        }
        #[entry(Playing)]
        fn entry_playing(_up: PlayingFields<'_>, _active: ActiveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Playing");
        }
        #[exit(Playing)]
        fn exit_playing(_up: PlayingFields<'_>, _active: ActiveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Playing");
        }
        #[entry(Normal)]
        fn entry_normal(_playing: PlayingFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Normal");
        }
        #[exit(Normal)]
        fn exit_normal(cx: &mut Cx<Self>) {
            note(cx, "exit Normal");
        }
        #[entry(Fast)]
        fn entry_fast(cx: &mut Cx<Self>) {
            note(cx, "entry Fast");
        }
        #[exit(Fast)]
        fn exit_fast(cx: &mut Cx<Self>) {
            note(cx, "exit Fast");
        }
        #[entry(Paused)]
        fn entry_paused(cx: &mut Cx<Self>) {
            note(cx, "entry Paused");
        }
        #[exit(Paused)]
        fn exit_paused(cx: &mut Cx<Self>) {
            note(cx, "exit Paused");
        }

        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler outside");
            match event {
                Event::ToActive => goto!(Active { count: 0 }),
                Event::ToActiveWithHistory => goto!(Active { count: 0 }, history),
                Event::ToActiveAtSpeed(speed) => {
                    goto!(Active { count: 0 } / Playing { speed: *speed })
                }
                _ => unhandled!(),
            }
        }

        #[on(Stopped)]
        fn on_stopped(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler outside");
            match event {
                Event::ToActive => goto!(Active { count: 0 }),
                Event::ToActiveWithHistory => goto!(Active { count: 0 }, history),
                _ => unhandled!(),
            }
        }

        #[on(Normal)]
        fn on_normal(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Normal");
            match event {
                Event::ToPaused => goto!(Paused),
                Event::NormalToFast => goto!(Fast),
                Event::NormalToActive => goto!(Active),
                Event::NormalToActiveExternal => goto!(Active { count: 100 }, external),
                Event::NormalToItself => goto!(Normal),
                _ => bubble!(),
            }
        }

        #[on(Playing)]
        fn on_playing(up: PlayingFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Playing");
            match event {
                Event::PlayingToIdle => goto!(Idle),
                Event::ToPaused => goto!(Paused),
                Event::ReenterPlaying => reenter!(),
                Event::SetSpeed(speed) => {
                    *up.speed = *speed;
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[on(Active)]
        fn on_active(up: ActiveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Active");
            match event {
                Event::ActiveToFast => goto!(Playing { speed: 1 } / Fast),
                Event::StayInActive => {
                    *up.count += 1;
                    stay!()
                }
                Event::ActiveToStopped => goto!(Stopped, |old: Exiting![Active]| {
                    note_free("construct Stopped");
                    Tree::Stopped { count: old.count }
                }),
                _ => bubble!(),
            }
        }

        #[on(Paused)]
        fn on_paused(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Paused");
            match event {
                Event::PausedToPlayingWithHistory => goto!(Playing { speed: 1 }, history),
                _ => bubble!(),
            }
        }
    }

    fn note_free(_line: &str) {}
}

pub use tree::{Active, ActiveSub, Playing, PlayingSub, Tree};

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
fn entering_a_compound_enters_its_initials() {
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
    assert_eq!(machine.path().to_string(), "Active/Playing/Normal");
}

#[test]
fn a_path_with_data_for_an_intermediate_level() {
    let mut machine = idle();
    machine.send(Event::ToActiveAtSpeed(9)).unwrap();
    assert_eq!(*machine.state(), playing(0, 9, PlayingSub::Normal));
}

#[test]
fn target_outside_the_handler_subtree_exits_up_to_the_lca() {
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
fn cousin_transition_stops_below_the_common_ancestor() {
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
fn local_transition_to_a_descendant_of_the_handler() {
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
fn local_transition_to_an_ancestor_keeps_the_ancestor() {
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
fn external_transition_to_an_ancestor_exits_and_enters_it() {
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
fn goto_the_handler_state_exits_and_enters_it() {
    let mut machine = normal();
    let report = machine.send(Event::NormalToItself).unwrap().report;
    assert_eq!(report.transitions, 1);
    assert_eq!(
        std::mem::take(machine.ctx_mut()),
        ["handler Normal", "exit Normal", "entry Normal"]
    );
}

#[test]
fn reenter_restarts_substates_and_keeps_own_data() {
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
fn stay_runs_no_exit_or_entry() {
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
fn constructor_form_takes_the_exiting_state_by_value() {
    let mut machine = normal();
    machine.send(Event::StayInActive).unwrap();
    machine.send(Event::StayInActive).unwrap();
    machine.ctx_mut().clear();
    assert_eq!(
        send(&mut machine, Event::ActiveToStopped),
        [
            "handler Normal",
            "handler Playing",
            "handler Active",
            "exit Normal",
            "exit Playing",
            "exit Active",
            "entry Stopped"
        ]
    );
    assert_eq!(*machine.state(), Tree::Stopped { count: 2 });
}

#[test]
fn a_state_override_of_the_unhandled_policy_stops_the_bubbling() {
    let mut machine = idle();
    machine.send(Event::ToActive).unwrap();
    machine.send(Event::ToPaused).unwrap();
    // `Paused` has `#[unhandled(error)]`: the event dies there, `Active`'s
    // handler is skipped, and the machine default (ignore) is overridden.
    machine.ctx_mut().clear();
    let error = machine.send(Event::UnknownEvent).unwrap_err();
    assert!(matches!(
        error,
        estado_core::SendError::Unhandled(Event::UnknownEvent)
    ));
}

#[test]
fn unhandled_at_the_top_follows_the_machine_default() {
    let mut machine = idle();
    let step = machine.send(Event::UnknownEvent).unwrap();
    assert_eq!(
        step.report.ignored,
        Some(estado_core::IgnoreReason::Unhandled)
    );
}

#[test]
fn path_and_is_in_follow_the_active_configuration() {
    let mut machine = normal();
    assert_eq!(machine.path().to_string(), "Active/Playing/Normal");
    assert!(machine.is_in(Tree::PLAYING_ID));
    machine.send(Event::ToPaused).unwrap();
    assert_eq!(machine.path().to_string(), "Active/Paused");
    assert!(!machine.is_in(Tree::PLAYING_ID));
}

#[test]
fn the_graph_is_the_union_of_the_edges() {
    use estado_core::{EdgeTarget, Spec};
    let graph = Tree::GRAPH;
    let edges_from = |from| {
        graph
            .edges
            .iter()
            .filter(move |edge| edge.from == from)
            .map(|edge| edge.to)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        edges_from(Tree::NORMAL_ID),
        [
            EdgeTarget::State(Tree::PAUSED_ID),
            EdgeTarget::State(Tree::FAST_ID),
            EdgeTarget::State(Tree::ACTIVE_ID),
            EdgeTarget::State(Tree::ACTIVE_ID),
            EdgeTarget::State(Tree::NORMAL_ID),
        ]
    );
    assert_eq!(graph.state(Tree::PLAYING_ID).initial, Some(Tree::NORMAL_ID));
}

#[test]
fn deep_history_without_a_record_uses_the_initial() {
    let mut machine = idle();
    machine.send(Event::ToActiveWithHistory).unwrap();
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Normal));
}

#[test]
fn deep_history_restores_the_full_substate_value() {
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
fn deep_history_is_recorded_when_a_constructor_takes_the_rest() {
    let mut machine = normal();
    machine.send(Event::NormalToFast).unwrap();
    machine.send(Event::ActiveToStopped).unwrap();
    assert_eq!(*machine.state(), Tree::Stopped { count: 0 });
    machine.send(Event::ToActiveWithHistory).unwrap();
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Fast));
}

#[test]
fn shallow_history_restores_the_substate_and_rebuilds_the_data() {
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
    // The data is rebuilt by the `goto!`: speed back to 1.
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Fast));
}

#[test]
fn a_plain_goto_ignores_recorded_history() {
    let mut machine = normal();
    machine.send(Event::NormalToFast).unwrap();
    machine.send(Event::PlayingToIdle).unwrap();
    machine.send(Event::ToActive).unwrap();
    assert_eq!(*machine.state(), playing(0, 1, PlayingSub::Normal));
}
