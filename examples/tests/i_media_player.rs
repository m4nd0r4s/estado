//! Scenario I (spec 11): nesting, the `*Fields` split borrow, and a
//! constructor-form edge that moves a non-`Clone` buffer into `Failed`.

#[path = "i_media_player/buffer_moves.rs"]
mod buffer_moves;
#[path = "i_media_player/support.rs"]
mod support;

use std::time::Duration;

use estado_core::{Effect, IgnoreReason, Machine, Spec};
use estado_examples::i_media_player::{ActiveSub, Command, Event, Player, TrackId};

use support::{SECOND, active_of, idle, paused, playing, send};

#[test]
fn start_enters_idle() {
    let mut machine = Machine::<Player>::new(Vec::new());
    let step = machine.start().unwrap();
    assert_eq!(step.effects.len(), 0);
    assert_eq!(*machine.ctx(), ["entry Idle"]);
    assert_eq!(machine.path().to_string(), "Idle");
}

#[test]
fn play_enters_active_and_its_initial_substate() {
    let mut machine = idle();
    let (log, commands) = send(&mut machine, Event::Play(TrackId(7)));
    assert_eq!(
        log,
        [
            "handler Idle",
            "exit Idle",
            "entry Active",
            "emit OpenTrack(TrackId(7))",
            "entry Playing",
            "emit StartOutput",
        ]
    );
    assert_eq!(
        commands,
        [Command::OpenTrack(TrackId(7)), Command::StartOutput]
    );
    assert_eq!(machine.path().to_string(), "Active/Playing");
    let active = active_of(&machine);
    assert_eq!(active.track, TrackId(7));
    assert_eq!(
        active.sub,
        ActiveSub::Playing {
            pos: Duration::ZERO
        }
    );
}

#[test]
fn pause_and_resume_keep_the_position() {
    let mut machine = playing();
    machine.send(Event::Tick(SECOND)).unwrap();
    machine.ctx_mut().clear();

    let (log, commands) = send(&mut machine, Event::Pause);
    assert_eq!(
        log,
        [
            "handler Playing",
            "exit Playing",
            "emit StopOutput",
            "entry Paused"
        ]
    );
    assert_eq!(commands, [Command::StopOutput]);
    assert_eq!(active_of(&machine).sub, ActiveSub::Paused { pos: SECOND });

    let (log, commands) = send(&mut machine, Event::Resume);
    assert_eq!(
        log,
        [
            "handler Paused",
            "exit Paused",
            "entry Playing",
            "emit StartOutput"
        ]
    );
    assert_eq!(commands, [Command::StartOutput]);
    assert_eq!(active_of(&machine).sub, ActiveSub::Playing { pos: SECOND });
}

#[test]
fn stop_exits_the_substate_then_active() {
    let mut machine = playing();
    let (log, commands) = send(&mut machine, Event::Stop);
    assert_eq!(
        log,
        [
            "handler Playing",
            "handler Active",
            "exit Playing",
            "emit StopOutput",
            "exit Active",
            "emit CloseTrack(TrackId(7))",
            "entry Idle"
        ]
    );
    assert_eq!(
        commands,
        [Command::StopOutput, Command::CloseTrack(TrackId(7))]
    );
    assert_eq!(*machine.state(), Player::Idle);
}

#[test]
fn stop_from_paused_has_no_output_to_stop() {
    let mut machine = paused();
    let (log, commands) = send(&mut machine, Event::Stop);
    assert_eq!(
        log,
        [
            "handler Paused",
            "handler Active",
            "exit Paused",
            "exit Active",
            "emit CloseTrack(TrackId(7))",
            "entry Idle"
        ]
    );
    assert_eq!(commands, [Command::CloseTrack(TrackId(7))]);
}

#[test]
fn child_handlers_mutate_the_fields_of_active_next_to_their_own_data() {
    let mut machine = playing();
    let (log, commands) = send(&mut machine, Event::Tick(SECOND));
    assert_eq!(log, ["handler Playing"]);
    assert!(commands.is_empty());
    let active = active_of(&machine);
    // `pos` through `PlayingMut`, `buf` through `ActiveFields`, in one handler.
    assert_eq!(active.sub, ActiveSub::Playing { pos: SECOND });
    assert_eq!(active.buf.decoded(), SECOND);

    let target = Duration::from_secs(30);
    let (log, commands) = send(&mut machine, Event::Seek(target));
    assert_eq!(
        log,
        [
            "handler Playing",
            "emit SeekOutput { track: TrackId(7), to: 30s }"
        ]
    );
    // The track id read through `ActiveFields` ends up in the command.
    assert_eq!(
        commands,
        [Command::SeekOutput {
            track: TrackId(7),
            to: target
        }]
    );
    let active = active_of(&machine);
    assert_eq!(active.sub, ActiveSub::Playing { pos: target });
    assert_eq!(active.buf.decoded(), target);
}

#[test]
fn seek_while_paused_moves_the_position_and_the_buffer_silently() {
    let mut machine = paused();
    let (log, commands) = send(&mut machine, Event::Seek(Duration::from_secs(5)));
    assert_eq!(log, ["handler Paused"]);
    assert!(commands.is_empty());
    let active = active_of(&machine);
    assert_eq!(
        active.sub,
        ActiveSub::Paused {
            pos: Duration::from_secs(5)
        }
    );
    assert_eq!(active.buf.decoded(), Duration::from_secs(5));
}

#[test]
fn playing_the_open_track_again_stays() {
    let mut machine = playing();
    let report = machine.send(Event::Play(TrackId(7))).unwrap().report;
    assert!(report.handled);
    assert_eq!(report.transitions, 0);
    assert_eq!(*machine.ctx(), ["handler Playing", "handler Active"]);
}

#[test]
fn unhandled_events_change_nothing_and_are_reported() {
    // (machine, event, path, trace of the handlers that saw it)
    let cases: [(Machine<Player>, Event, &str, &[&str]); 5] = [
        (idle(), Event::Pause, "Idle", &["handler Idle"]),
        (
            playing(),
            Event::Resume,
            "Active/Playing",
            &["handler Playing", "handler Active"],
        ),
        (
            paused(),
            Event::Tick(SECOND),
            "Active/Paused",
            &["handler Paused", "handler Active"],
        ),
        (
            paused(),
            Event::Pause,
            "Active/Paused",
            &["handler Paused", "handler Active"],
        ),
        (
            {
                let mut machine = playing();
                machine.send(Event::Error("x".to_owned())).unwrap();
                machine.ctx_mut().clear();
                machine
            },
            Event::Tick(SECOND),
            "Failed",
            &["handler Failed"],
        ),
    ];
    for (mut machine, event, path, trace) in cases {
        let step = machine.send(event.clone()).unwrap();
        assert!(!step.report.handled, "{event:?} in {path}");
        assert_eq!(step.report.ignored, Some(IgnoreReason::Unhandled));
        assert_eq!(step.report.transitions, 0);
        assert_eq!(step.effects.len(), 0);
        assert_eq!(*machine.ctx(), trace, "{event:?} in {path}");
        assert_eq!(machine.path().to_string(), path);
    }
}

#[test]
fn an_unhandled_tick_while_paused_does_not_decode() {
    let mut machine = paused();
    let before = active_of(&machine).buf.decoded();
    machine.send(Event::Tick(SECOND)).unwrap();
    assert_eq!(active_of(&machine).buf.decoded(), before);
}

#[test]
fn path_display_and_state_ids_follow_the_configuration() {
    let mut machine = idle();
    assert_eq!(machine.path().to_string(), "Idle");
    assert!(machine.is_in(Player::IDLE));

    machine.send(Event::Play(TrackId(1))).unwrap();
    assert_eq!(machine.path().to_string(), "Active/Playing");
    assert!(machine.is_in(Player::ACTIVE) && machine.is_in(Player::PLAYING));
    assert_eq!(machine.state().state_id(), Player::PLAYING);

    machine.send(Event::Pause).unwrap();
    assert_eq!(machine.path().to_string(), "Active/Paused");
    assert!(machine.is_in(Player::PAUSED) && !machine.is_in(Player::PLAYING));

    machine.send(Event::Error("e".to_owned())).unwrap();
    assert_eq!(machine.path().to_string(), "Failed");
    assert!(machine.is_in(Player::FAILED) && !machine.is_in(Player::ACTIVE));
}

#[test]
fn the_graph_describes_the_hierarchy() {
    let graph = Player::GRAPH;
    assert_eq!(graph.machine, "Player");
    let playing = graph.find("Playing").unwrap();
    assert_eq!(playing.parent, Some(Player::ACTIVE));
    assert_eq!(graph.state(Player::ACTIVE).initial, Some(Player::PLAYING));
    assert!(graph.is_ancestor_or_self(Player::ACTIVE, Player::PAUSED));
    assert!(!graph.is_ancestor_or_self(Player::ACTIVE, Player::FAILED));
}

#[test]
fn a_whole_session_in_one_send_batch() {
    let mut machine = idle();
    let mut effects = Vec::new();
    let report = machine
        .send_batch(
            [
                Event::Play(TrackId(3)),
                Event::Tick(SECOND),
                Event::Pause,
                Event::Resume,
                Event::Stop,
            ],
            &mut effects,
        )
        .unwrap();
    assert_eq!(report.transitions, 4);
    let commands: Vec<_> = effects
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::User(command) => Some(command),
            _ => None,
        })
        .collect();
    assert_eq!(
        commands,
        [
            Command::OpenTrack(TrackId(3)),
            Command::StartOutput,
            Command::StopOutput,
            Command::StartOutput,
            Command::StopOutput,
            Command::CloseTrack(TrackId(3)),
        ]
    );
    assert_eq!(*machine.state(), Player::Idle);
}
