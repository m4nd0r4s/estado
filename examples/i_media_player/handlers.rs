//! The user's handlers, as they look after `#[handlers]` rewrote each `goto!`
//! into a variant of the handler's edge enum (spec 6.2, 6.3, ADR 0001).
//!
//! Parameters come in the order of spec 6.2: own data, the `*Fields` of the
//! ancestors, `cx`, the event. A handler that never returns a constructor
//! edge names its edge enum; one that does returns `impl Edge` with precise
//! capturing, so the constructor may borrow from the event and nothing else.

use std::time::Duration;

use estado_core::{Edge, Next, Sink};

use super::actions::{PlayerCx, emit, note};
use super::edges::{ActiveEdges, FailedEdges, IdleEdges, PausedEdges, PlayingEdges};
use super::model::{Active, ActiveSub, AudioBuffer, Command, Event, Player};
use super::views::{ActiveExiting, ActiveFields, FailedExiting, FailedMut, PausedMut, PlayingMut};

impl ActiveFields<'_> {
    /// Moves the decode position of the buffer.
    ///
    /// A helper on the fields view, not on `Active`: a child handler holds
    /// `&mut` to its own substate, so `Active::seek(&mut self, ..)` is out of
    /// reach there (risk R3).
    fn seek_buffer(&mut self, to: Duration) {
        self.buf.seek(to);
    }
}

/// `#[on(Idle)]`.
pub fn on_idle<S: Sink<Command>>(
    cx: &mut PlayerCx<'_, S>,
    event: &Event,
) -> Next<Player, IdleEdges> {
    note(cx, "handler Idle");
    match event {
        Event::Play(track) => Next::Goto(IdleEdges::ToActive {
            track: *track,
            buf: AudioBuffer::for_track(*track),
        }),
        _ => Next::Bubble,
    }
}

/// `#[on(Playing)]`: own data through `PlayingMut`, the `buf` of `Active`
/// through `ActiveFields`, both mutable at the same time.
pub fn on_playing<S: Sink<Command>>(
    playing: PlayingMut<'_>,
    mut up: ActiveFields<'_>,
    cx: &mut PlayerCx<'_, S>,
    event: &Event,
) -> Next<Player, PlayingEdges> {
    note(cx, "handler Playing");
    match event {
        Event::Tick(elapsed) => {
            *playing.pos += *elapsed;
            up.buf.decode(*elapsed);
            Next::Stay
        }
        Event::Seek(target) => {
            *playing.pos = *target;
            up.seek_buffer(*target);
            emit(
                cx,
                Command::SeekOutput {
                    track: *up.track,
                    to: *target,
                },
            );
            Next::Stay
        }
        // The position is copied into the edge before the state is touched.
        Event::Pause => Next::Goto(PlayingEdges::ToPaused { pos: *playing.pos }),
        _ => Next::Bubble,
    }
}

/// `#[on(Paused)]`.
pub fn on_paused<S: Sink<Command>>(
    paused: PausedMut<'_>,
    mut up: ActiveFields<'_>,
    cx: &mut PlayerCx<'_, S>,
    event: &Event,
) -> Next<Player, PausedEdges> {
    note(cx, "handler Paused");
    match event {
        Event::Seek(target) => {
            *paused.pos = *target;
            up.seek_buffer(*target);
            Next::Stay
        }
        Event::Resume => Next::Goto(PausedEdges::ToPlaying { pos: *paused.pos }),
        _ => Next::Bubble,
    }
}

/// `#[on(Active)]`: the handler of the compound state itself. It receives the
/// fields of `Active` but not its substate: replacing `sub` here would skip
/// the exits and entries.
pub fn on_active<'event, S: Sink<Command>>(
    up: ActiveFields<'_>,
    cx: &mut PlayerCx<'_, S>,
    event: &'event Event,
) -> Next<Player, impl Edge<Player> + use<'event, S>> {
    note(cx, "handler Active");
    match event {
        Event::Play(track) if track == &*up.track => Next::Stay,
        Event::Play(track) => Next::Goto(ActiveEdges::ToActive {
            track: *track,
            buf: AudioBuffer::for_track(*track),
        }),
        Event::Stop => Next::Goto(ActiveEdges::ToIdle),
        // The constructor borrows `message` from the event and moves `buf`
        // out of the exiting `Active`: no `Clone` on the buffer.
        Event::Error(message) => {
            Next::Goto(ActiveEdges::ToFailed(|old: ActiveExiting| Player::Failed {
                reason: message.clone(),
                buffer: old.buf,
            }))
        }
        _ => Next::Bubble,
    }
}

/// `#[on(Failed)]`.
pub fn on_failed<'event, S: Sink<Command>>(
    failed: FailedMut<'_>,
    cx: &mut PlayerCx<'_, S>,
    event: &'event Event,
) -> Next<Player, impl Edge<Player> + use<'event, S>> {
    note(cx, "handler Failed");
    match event {
        // Retry: the buffer moves back into a new `Active`, rewound.
        Event::Play(track) => Next::Goto(FailedEdges::ToActive(|old: FailedExiting| {
            Player::Active(Active {
                track: *track,
                buf: old.buffer.rewound(),
                sub: ActiveSub::initial(),
            })
        })),
        Event::Stop => Next::Goto(FailedEdges::ToIdle),
        Event::Error(message) => {
            failed.reason.clone_from(message);
            emit(cx, Command::ShowError(message.clone()));
            Next::Stay
        }
        _ => Next::Bubble,
    }
}
