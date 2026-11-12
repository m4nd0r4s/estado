//! Generated views (spec 6.2, 6.3): the split borrow of `Active` and its
//! substates, views of struct-like variants and the types of `Exiting![..]`.

use std::time::Duration;

use super::model::{Active, ActiveSub, AudioBuffer, TrackId};

/// The fields of `Active` without its `#[substates]` field.
///
/// Handlers of the substates receive it next to their own data, so both
/// borrows are alive at once without aliasing.
///
/// This is also the limit of the technique (risk R3): a handler of a
/// substate can never have the whole `&mut Active`, because its own data
/// lives inside `Active::sub`. The dispatch below cannot be written:
///
/// ```compile_fail,E0499
/// use estado_examples::i_media_player::{Active, ActiveSub, PlayingMut};
///
/// fn handler(_playing: PlayingMut<'_>, _whole: &mut Active) {}
///
/// fn dispatch(active: &mut Active) {
///     if let ActiveSub::Playing { pos } = &mut active.sub {
///         handler(PlayingMut { pos }, active);
///     }
/// }
/// ```
pub struct ActiveFields<'a> {
    /// `Active::track`.
    pub track: &'a mut TrackId,
    /// `Active::buf`.
    pub buf: &'a mut AudioBuffer,
}

impl Active {
    /// Splits the borrow: the fields of `Active` and its substate.
    pub fn split(&mut self) -> (ActiveFields<'_>, &mut ActiveSub) {
        let Self { track, buf, sub } = self;
        (ActiveFields { track, buf }, sub)
    }
}

impl ActiveFields<'_> {
    /// A shorter-lived copy, to hand the fields to a helper and keep using
    /// them afterwards.
    pub fn reborrow(&mut self) -> ActiveFields<'_> {
        ActiveFields {
            track: self.track,
            buf: self.buf,
        }
    }
}

/// View of `ActiveSub::Playing { pos }`.
pub struct PlayingMut<'a> {
    /// `pos` of the variant.
    pub pos: &'a mut Duration,
}

/// View of `ActiveSub::Paused { pos }`.
pub struct PausedMut<'a> {
    /// `pos` of the variant.
    pub pos: &'a mut Duration,
}

/// View of `Player::Failed { reason, buffer }`.
pub struct FailedMut<'a> {
    /// `reason` of the variant.
    pub reason: &'a mut String,
    /// `buffer` of the variant.
    pub buffer: &'a mut AudioBuffer,
}

/// `Exiting![Active]`: the child of the LCA when `Active` leaves towards the
/// root. `Active` has no deep history, so it is `Active` itself, substates
/// included (spec 6.3).
pub type ActiveExiting = Active;

/// `Exiting![Failed]`: a struct-like variant has no named type of its own, so
/// the macro generates an owned struct with its fields.
pub struct FailedExiting {
    /// `reason` of the variant.
    pub reason: String,
    /// `buffer` of the variant.
    pub buffer: AudioBuffer,
}

impl ActiveSub {
    /// The `#[initial]` substate, with its data.
    pub fn initial() -> Self {
        Self::Playing {
            pos: Duration::ZERO,
        }
    }
}
