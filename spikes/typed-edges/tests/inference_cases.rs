//! Behaviour of the hand-generated typed-edge code.

use typed_edges_spike::core::{Context, Edge};
use typed_edges_spike::dispatch::{Outcome, microstep};
use typed_edges_spike::edges::{ActiveEdges, PlayingEdges};
use typed_edges_spike::model::{Active, ActiveSub, Event, Player};

fn playing(track: u32, position: u32) -> Player {
    Player::Active(Active {
        track,
        buffer: vec![1, 2, 3],
        sub: ActiveSub::Playing { position },
    })
}

fn run(machine: &mut Player, event: Event) -> (Outcome, Vec<String>) {
    let mut context = Context::default();
    let outcome = microstep(machine, &mut context, &event);
    (outcome, context.log)
}

#[test]
fn simple_local_edge_into_sibling_substate() {
    let mut machine = playing(7, 2);
    let (outcome, log) = run(&mut machine, Event::Pause);
    assert_eq!(outcome, Outcome::Transitioned);
    assert_eq!(log, ["handler Playing", "exit Playing", "entry Paused"]);
    assert_eq!(machine, {
        let mut expected = playing(7, 2);
        if let Player::Active(active) = &mut expected {
            active.sub = ActiveSub::Paused { position: 2 };
        }
        expected
    });
}

#[test]
fn constructor_borrowing_the_event_moves_the_buffer_without_clone() {
    let mut machine = playing(7, 0);
    let (_, log) = run(&mut machine, Event::Error("disk".into()));
    assert_eq!(
        log,
        [
            "handler Playing",
            "exit Playing",
            "exit Active",
            "entry Failed"
        ]
    );
    assert_eq!(
        machine,
        Player::Failed {
            reason: "disk".into(),
            buffer: vec![1, 2, 3]
        }
    );
}

#[test]
fn early_return_branch() {
    let mut machine = playing(7, 3);
    let (outcome, log) = run(&mut machine, Event::Tick);
    assert_eq!(outcome, Outcome::Transitioned);
    assert_eq!(log.last().map(String::as_str), Some("entry Idle"));
    assert_eq!(machine, Player::Idle);
}

#[test]
fn branch_that_mutates_and_stays() {
    let mut machine = playing(7, 0);
    assert_eq!(run(&mut machine, Event::Tick).0, Outcome::Stayed);
    assert_eq!(machine, playing(7, 1));
}

#[test]
fn nested_match_with_guard_reads_parent_fields() {
    let mut machine = playing(7, 0);
    assert_eq!(run(&mut machine, Event::Play(7)).0, Outcome::Stayed);
    run(&mut machine, Event::Play(9));
    assert!(
        matches!(machine, Player::Failed { ref reason, .. } if reason == "track change while playing")
    );
}

#[test]
fn goto_inside_a_closure_with_a_move_constructor() {
    let mut machine = playing(7, 0);
    assert_eq!(run(&mut machine, Event::MaybeStop(None)).0, Outcome::Stayed);
    run(&mut machine, Event::MaybeStop(Some(4)));
    assert!(matches!(machine, Player::Failed { ref reason, .. } if reason == "stopped with 4"));
}

#[test]
fn bubble_into_a_result_handler_with_question_mark() {
    let mut machine = playing(7, 0);
    let (outcome, log) = run(&mut machine, Event::Seek("42"));
    assert_eq!(outcome, Outcome::Transitioned);
    assert_eq!(
        log,
        [
            "handler Playing",
            "handler Active",
            "exit Playing",
            "entry Playing"
        ]
    );
    assert_eq!(machine, playing(7, 42));

    let (outcome, _) = run(&mut machine, Event::Seek("not a number"));
    assert_eq!(outcome, Outcome::HandlerFailed);
}

#[test]
fn break_with_value_from_a_loop_and_external_reentry() {
    let mut machine = playing(7, 5);
    let mut context = Context::default();
    // From Paused so the event reaches `Active` (Playing handles `Play`).
    if let Player::Active(active) = &mut machine {
        active.sub = ActiveSub::Paused { position: 5 };
    }
    microstep(&mut machine, &mut context, &Event::Play(3));
    assert_eq!(
        context.log,
        [
            "handler Paused",
            "handler Active",
            "exit Paused",
            "exit Active",
            "entry Active",
            "entry Playing"
        ]
    );
    assert_eq!(machine, playing(3, 0));
}

#[test]
fn unhandled_from_the_top_of_the_hierarchy() {
    let mut machine = playing(7, 0);
    assert_eq!(run(&mut machine, Event::Resume).0, Outcome::Transitioned);
    let mut idle = Player::Idle;
    assert_eq!(run(&mut idle, Event::Pause).0, Outcome::Unhandled);
}

#[test]
fn edge_sets_expose_their_targets_for_the_graph() {
    type PlayingSet =
        PlayingEdges<fn(Active) -> Player, fn(Active) -> Player, fn(Active) -> Player>;
    assert_eq!(
        <PlayingSet as Edge<Player>>::TARGETS,
        ["Paused", "Idle", "Failed", "Failed", "Failed"]
    );
    assert_eq!(
        <ActiveEdges as Edge<Player>>::TARGETS,
        ["Playing", "Active"]
    );
}
