//! The media player as a `#[machine]` module (ADR 0010).
//!
//! Compared with the hand-written scenario: the enums and structs are the
//! same; `#[handlers]` replaces the dispatch, the edge enums, the entries and
//! exits, the views and the graph, all generated.

use std::time::Duration;

use estado_core::{Cx, Next, Sink};

use super::{AudioBuffer, Command, Event};

/// Appends a line to the trace kept in the global context.
fn note<S: Sink<Command>>(cx: &mut Cx<'_, player::Player, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

/// Traces and emits a command.
fn emit<S: Sink<Command>>(cx: &mut Cx<'_, player::Player, S>, command: Command) {
    let line = format!("emit {command:?}");
    cx.ctx_mut().push(line);
    cx.emit(command);
}

#[estado_macros::machine(event = Event, effect = Command, ctx = Vec<String>)]
pub mod player {
    use super::*;
    use crate::i_media_player::TrackId;

    /// The machine: root enum.
    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        /// `#[initial]`.
        #[initial]
        Idle,
        /// Compound state.
        Active(Active),
        /// Atomic state with data; it keeps the buffer that `Active` gave up.
        Failed {
            /// Message of the error event that caused the failure.
            reason: String,
            /// The audio buffer, moved here without `Clone`.
            buffer: AudioBuffer,
        },
    }

    /// Compound state: shared data and the substates.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        /// Track that is open.
        pub track: TrackId,
        /// Decoded audio; lives as long as `Active` exists.
        pub buf: AudioBuffer,
        /// `#[substates]`.
        #[substates]
        pub sub: ActiveSub,
    }

    /// Substates of `Active`, struct-like variants on purpose.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ActiveSub {
        /// Playing from the position `0`.
        #[initial(with = start_playing)]
        Playing {
            /// Playback position.
            pos: Duration,
        },
        /// Output stopped, position kept.
        Paused {
            /// Playback position.
            pos: Duration,
        },
    }

    /// `#[initial(with = ..)]` of `Playing`: it carries data.
    fn start_playing() -> ActiveSub {
        ActiveSub::Playing {
            pos: Duration::ZERO,
        }
    }

    #[handlers]
    impl Player {
        // Entries and exits, one per state.

        #[entry(Idle)]
        fn entry_idle(cx: &mut Cx<Self>) {
            note(cx, "entry Idle");
        }

        #[exit(Idle)]
        fn exit_idle(cx: &mut Cx<Self>) {
            note(cx, "exit Idle");
        }

        #[entry(Active)]
        fn entry_active(up: ActiveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Active");
            emit(cx, Command::OpenTrack(*up.track));
        }

        #[exit(Active)]
        fn exit_active(up: ActiveFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Active");
            emit(cx, Command::CloseTrack(*up.track));
        }

        #[entry(Playing)]
        fn entry_playing(_playing: PlayingMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Playing");
            emit(cx, Command::StartOutput);
        }

        #[exit(Playing)]
        fn exit_playing(_playing: PlayingMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Playing");
            emit(cx, Command::StopOutput);
        }

        #[entry(Paused)]
        fn entry_paused(_paused: PausedMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Paused");
        }

        #[exit(Paused)]
        fn exit_paused(_paused: PausedMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Paused");
        }

        #[entry(Failed)]
        fn entry_failed(failed: FailedMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Failed");
            emit(cx, Command::ShowError(failed.reason.clone()));
        }

        #[exit(Failed)]
        fn exit_failed(_failed: FailedMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Failed");
        }

        // Event handlers.

        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Idle");
            match event {
                Event::Play(track) => goto!(Active {
                    track: *track,
                    buf: AudioBuffer::for_track(*track),
                }),
                _ => bubble!(),
            }
        }

        /// Own data through `PlayingMut`, the `buf` of `Active` through
        /// `ActiveFields`, both mutable at the same time.
        #[on(Playing)]
        fn on_playing(
            playing: PlayingMut<'_>,
            up: ActiveFields<'_>,
            cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            note(cx, "handler Playing");
            match event {
                Event::Tick(elapsed) => {
                    *playing.pos += *elapsed;
                    up.buf.decode(*elapsed);
                    stay!()
                }
                Event::Seek(target) => {
                    *playing.pos = *target;
                    up.buf.seek(*target);
                    emit(
                        cx,
                        Command::SeekOutput {
                            track: *up.track,
                            to: *target,
                        },
                    );
                    stay!()
                }
                // The position is copied into the edge before the state is touched.
                Event::Pause => goto!(Paused { pos: *playing.pos }),
                _ => bubble!(),
            }
        }

        #[on(Paused)]
        fn on_paused(
            paused: PausedMut<'_>,
            up: ActiveFields<'_>,
            cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            note(cx, "handler Paused");
            match event {
                Event::Seek(target) => {
                    *paused.pos = *target;
                    up.buf.seek(*target);
                    stay!()
                }
                Event::Resume => goto!(Playing { pos: *paused.pos }),
                _ => bubble!(),
            }
        }

        /// The handler of the compound state itself: it receives the fields of
        /// `Active` but not its substate.
        #[on(Active)]
        fn on_active(up: ActiveFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Active");
            match event {
                Event::Play(track) if track == &*up.track => stay!(),
                Event::Play(track) => goto!(Active {
                    track: *track,
                    buf: AudioBuffer::for_track(*track),
                }),
                Event::Stop => goto!(Idle),
                // The constructor borrows `message` from the event and moves
                // `buf` out of the exiting `Active`: no `Clone` on the buffer.
                Event::Error(message) => goto!(Failed, |old: Exiting![Active]| Player::Failed {
                    reason: message.clone(),
                    buffer: old.buf,
                }),
                _ => bubble!(),
            }
        }

        #[on(Failed)]
        fn on_failed(failed: FailedMut<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Failed");
            match event {
                // Retry: the buffer moves back into a new `Active`, rewound.
                Event::Play(track) => {
                    goto!(Active, |old: Exiting![Failed]| Player::Active(Active {
                        track: *track,
                        buf: old.buffer.rewound(),
                        sub: start_playing(),
                    }))
                }
                Event::Stop => goto!(Idle),
                Event::Error(message) => {
                    failed.reason.clone_from(message);
                    emit(cx, Command::ShowError(message.clone()));
                    stay!()
                }
                _ => bubble!(),
            }
        }
    }
}
