//! The user's own types: states, events and commands (spec 3.2).

use std::time::Duration;

/// Identifies a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackId(pub u32);

/// Decoded audio of the open track.
///
/// Deliberately **not** `Clone`: it owns a large allocation, and the point of
/// the scenario is that a transition moves it instead of copying it.
#[derive(Debug, PartialEq, Eq)]
pub struct AudioBuffer {
    samples: Vec<u8>,
    decoded: Duration,
}

impl AudioBuffer {
    /// Opens the buffer of `track`, with content derived from the track id.
    pub fn for_track(track: TrackId) -> Self {
        let seed = track.0.to_le_bytes()[0];
        Self {
            samples: (0u8..32).map(|index| index.wrapping_add(seed)).collect(),
            decoded: Duration::ZERO,
        }
    }

    /// Decodes `by` more audio.
    pub fn decode(&mut self, by: Duration) {
        self.decoded += by;
    }

    /// Moves the decode position to `to`.
    pub fn seek(&mut self, to: Duration) {
        self.decoded = to;
    }

    /// Starts decoding from the beginning, keeping the allocation.
    pub fn rewound(mut self) -> Self {
        self.decoded = Duration::ZERO;
        self
    }

    /// How far the audio is decoded.
    pub fn decoded(&self) -> Duration {
        self.decoded
    }

    /// The raw samples.
    pub fn samples(&self) -> &[u8] {
        &self.samples
    }

    /// Address of the allocation, to tell a move from a copy.
    pub fn identity(&self) -> *const u8 {
        self.samples.as_ptr()
    }
}

/// The machine: root enum.
#[derive(Debug, PartialEq, Eq)]
pub enum Player {
    /// `#[initial]`.
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

/// Compound state: shared data and the substates (`#[state]`).
#[derive(Debug, PartialEq, Eq)]
pub struct Active {
    /// Track that is open.
    pub track: TrackId,
    /// Decoded audio; lives as long as `Active` exists.
    pub buf: AudioBuffer,
    /// `#[substates]`.
    pub sub: ActiveSub,
}

/// Substates of `Active` (`#[state]`), struct-like variants on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSub {
    /// `#[initial]`.
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

/// Events of the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Open a track and start playing it.
    Play(TrackId),
    /// Stop the output, keep the position.
    Pause,
    /// Leave `Paused`.
    Resume,
    /// Time passed while playing.
    Tick(Duration),
    /// Jump to a position.
    Seek(Duration),
    /// Close the track and go back to `Idle`.
    Stop,
    /// Something went wrong; the message becomes the failure reason.
    Error(String),
}

/// Effects (commands) the machine emits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Open the track in the audio backend.
    OpenTrack(TrackId),
    /// Close the track in the audio backend.
    CloseTrack(TrackId),
    /// Start the audio output.
    StartOutput,
    /// Stop the audio output.
    StopOutput,
    /// Move the audio output to a position.
    SeekOutput {
        /// Track that is open.
        track: TrackId,
        /// New position.
        to: Duration,
    },
    /// Show an error to the user.
    ShowError(String),
}
