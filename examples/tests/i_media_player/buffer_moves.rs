//! The buffer moves: `Active -> Failed` and `Failed -> Active`, both through
//! constructor-form edges, and the restart of `Active` with a new buffer.

use std::time::Duration;

use estado_examples::i_media_player::{ActiveSub, AudioBuffer, Command, Event, Player, TrackId};

use super::support::{SECOND, active_of, failed_of, paused, playing, send};

#[test]
fn an_error_moves_the_buffer_into_failed_without_cloning_it() {
    let mut machine = playing();
    machine.send(Event::Tick(SECOND)).unwrap();
    let before = active_of(&machine);
    let identity = before.buf.identity();
    let samples = before.buf.samples().to_vec();
    machine.ctx_mut().clear();

    let (log, commands) = send(&mut machine, Event::Error("disk unplugged".to_owned()));
    assert_eq!(
        log,
        [
            "handler Playing",
            "handler Active",
            "exit Playing",
            "emit StopOutput",
            "exit Active",
            "emit CloseTrack(TrackId(7))",
            "construct Failed",
            "entry Failed",
            "emit ShowError(\"disk unplugged\")",
        ]
    );
    assert_eq!(
        commands,
        [
            Command::StopOutput,
            Command::CloseTrack(TrackId(7)),
            Command::ShowError("disk unplugged".to_owned()),
        ]
    );
    let (reason, buffer) = failed_of(&machine);
    assert_eq!(reason, "disk unplugged");
    // The very same allocation, with the progress made before the error.
    assert_eq!(buffer.identity(), identity);
    assert_eq!(buffer.samples(), samples);
    assert_eq!(buffer.decoded(), SECOND);
    assert_eq!(machine.path().to_string(), "Failed");
}

#[test]
fn an_error_while_paused_skips_the_output_stop() {
    let mut machine = paused();
    let (log, _) = send(&mut machine, Event::Error("late".to_owned()));
    assert_eq!(
        log,
        [
            "handler Paused",
            "handler Active",
            "exit Paused",
            "exit Active",
            "emit CloseTrack(TrackId(7))",
            "construct Failed",
            "entry Failed",
            "emit ShowError(\"late\")",
        ]
    );
    assert_eq!(failed_of(&machine).0, "late");
}

#[test]
fn retry_moves_the_buffer_out_of_failed_into_a_new_active() {
    let mut machine = playing();
    machine.send(Event::Tick(SECOND)).unwrap();
    let identity = active_of(&machine).buf.identity();
    machine.send(Event::Error("glitch".to_owned())).unwrap();
    assert_eq!(failed_of(&machine).1.identity(), identity);
    machine.ctx_mut().clear();

    let (log, commands) = send(&mut machine, Event::Play(TrackId(9)));
    assert_eq!(
        log,
        [
            "handler Failed",
            "exit Failed",
            "construct Active",
            "entry Active",
            "emit OpenTrack(TrackId(9))",
            "entry Playing",
            "emit StartOutput",
        ]
    );
    assert_eq!(
        commands,
        [Command::OpenTrack(TrackId(9)), Command::StartOutput]
    );
    let active = active_of(&machine);
    assert_eq!(active.track, TrackId(9));
    assert_eq!(active.buf.identity(), identity, "moved twice, never copied");
    assert_eq!(
        active.buf.decoded(),
        Duration::ZERO,
        "the constructor rewound it"
    );
    assert_eq!(
        active.sub,
        ActiveSub::Playing {
            pos: Duration::ZERO
        }
    );
}

#[test]
fn failed_updates_its_reason_through_its_view() {
    let mut machine = playing();
    machine.send(Event::Error("first".to_owned())).unwrap();
    machine.ctx_mut().clear();
    let (log, commands) = send(&mut machine, Event::Error("second".to_owned()));
    assert_eq!(log, ["handler Failed", "emit ShowError(\"second\")"]);
    assert_eq!(commands, [Command::ShowError("second".to_owned())]);
    assert_eq!(failed_of(&machine).0, "second");
}

#[test]
fn stop_in_failed_drops_the_buffer_and_goes_idle() {
    let mut machine = playing();
    machine.send(Event::Error("x".to_owned())).unwrap();
    machine.ctx_mut().clear();
    let (log, commands) = send(&mut machine, Event::Stop);
    assert_eq!(log, ["handler Failed", "exit Failed", "entry Idle"]);
    assert!(commands.is_empty());
    assert_eq!(*machine.state(), Player::Idle);
}

#[test]
fn playing_another_track_restarts_active_with_a_new_buffer() {
    let mut machine = playing();
    let (log, commands) = send(&mut machine, Event::Play(TrackId(8)));
    assert_eq!(
        log,
        [
            "handler Playing",
            "handler Active",
            "exit Playing",
            "emit StopOutput",
            "exit Active",
            "emit CloseTrack(TrackId(7))",
            "entry Active",
            "emit OpenTrack(TrackId(8))",
            "entry Playing",
            "emit StartOutput",
        ]
    );
    assert_eq!(
        commands,
        [
            Command::StopOutput,
            Command::CloseTrack(TrackId(7)),
            Command::OpenTrack(TrackId(8)),
            Command::StartOutput
        ]
    );
    let active = active_of(&machine);
    assert_eq!(active.track, TrackId(8));
    assert_ne!(
        active.buf.samples(),
        AudioBuffer::for_track(TrackId(7)).samples()
    );
}
