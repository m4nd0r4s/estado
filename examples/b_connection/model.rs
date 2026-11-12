//! The user's own types: states, events, commands and timing constants
//! (spec 3.2, 3.3).

use std::time::Duration;

use estado_core::DeferOverflowPolicy;

/// How long `Connecting` waits for the dial result (`#[after]`).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// First backoff delay; it doubles with every failed attempt.
pub const BACKOFF_BASE: Duration = Duration::from_millis(100);
/// The backoff delay never grows beyond this.
pub const BACKOFF_CAP: Duration = Duration::from_secs(1);
/// Dials per connection cycle; the last timeout gives up.
pub const MAX_ATTEMPTS: u8 = 6;
/// Quiet time of `Pinging` before a ping is sent (`#[after]`).
pub const PING_INTERVAL: Duration = Duration::from_secs(5);
/// How long `AwaitingPong` waits for the pong (`#[after]`).
pub const PONG_TIMEOUT: Duration = Duration::from_secs(2);

/// `#[machine(defer_cap = 4)]`: deferred events kept across `send`s.
pub const DEFER_CAP: usize = 4;
/// `#[machine(raise_cap = 8)]`, the default.
pub const RAISE_CAP: usize = 8;
/// `#[machine(defer_overflow = error)]`, the default, spelled out: a `Send`
/// that does not fit in the defer queue is refused with
/// `SendError::DeferOverflow(event)`, the state untouched, so the caller can
/// slow down and resend. `DropOldest` would instead lose the oldest frame
/// silently, which is wrong for a byte stream with ordering requirements.
pub const DEFER_OVERFLOW_POLICY: DeferOverflowPolicy = DeferOverflowPolicy::Error;

/// Delay before attempt `attempt + 1`: 100 ms doubled per failed attempt,
/// capped at [`BACKOFF_CAP`].
pub fn backoff(attempt: u8) -> Duration {
    BACKOFF_BASE
        .saturating_mul(1u32 << u32::from(attempt).min(16))
        .min(BACKOFF_CAP)
}

/// The machine: root enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conn {
    /// `#[initial]`.
    Disconnected,
    /// `#[after(CONNECT_TIMEOUT => ConnectTimeout)]` and
    /// `#[invoke(|s| Command::Dial { attempt: s.attempt })]`.
    Connecting {
        /// Failed dials so far in this cycle.
        attempt: u8,
    },
    /// `#[after(backoff(attempt) => Retry)]`.
    Backoff {
        /// Failed dials so far in this cycle.
        attempt: u8,
    },
    /// Parallel state with the regions `link` and `keepalive`.
    Connected(Connected),
}

/// Parallel state: shared data and two `#[region]`s, in evaluation order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connected {
    /// The session the dial produced.
    pub session: Session,
    /// Frames the link has delivered.
    pub frames_sent: u32,
    /// First region.
    pub link: Link,
    /// Second region.
    pub keepalive: Keepalive,
}

impl Connected {
    /// `goto!(Connected { session })`: each region starts in its `#[initial]`.
    pub fn fresh(session: Session) -> Self {
        Self {
            session,
            frames_sent: 0,
            link: Link::Idle,
            keepalive: Keepalive::Pinging,
        }
    }
}

/// An established session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    /// Session id given by the peer.
    pub id: u32,
}

/// Region `link`: one frame in flight at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// `#[initial]`.
    Idle,
    /// `#[invoke(|s| Command::SendFrame(s.frame.clone()))]`.
    Sending {
        /// The frame in flight.
        frame: Frame,
    },
}

/// Region `keepalive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keepalive {
    /// `#[initial]`, `#[after(PING_INTERVAL => PingDue)]`.
    Pinging,
    /// `#[after(PONG_TIMEOUT => PongTimeout)]`; its entry emits `Ping`.
    AwaitingPong,
    /// The peer is gone; its entry raises `KeepaliveDead`.
    Dead,
}

/// Bytes to send; owned, because deferred events must be (spec 5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame(pub Vec<u8>);

impl Frame {
    /// A frame holding `bytes`.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }
}

/// Events of the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Start connecting.
    Connect,
    /// Result of the dial invoke.
    Connected(Session),
    /// The connect timer of `Connecting`.
    ConnectTimeout,
    /// The backoff timer of `Backoff`.
    Retry,
    /// Hang up.
    Disconnect,
    /// Send a frame: deferred until the link can take it.
    Send(Frame),
    /// Result of the send invoke.
    FrameSent,
    /// The peer answered a ping.
    Pong,
    /// The ping timer of `Pinging`.
    PingDue,
    /// The pong timer of `AwaitingPong`.
    PongTimeout,
    /// Raised by the entry of `Dead`.
    KeepaliveDead,
}

/// Effects (commands) the machine emits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// The task of `Connecting`: open the transport.
    Dial {
        /// Failed dials so far in this cycle.
        attempt: u8,
    },
    /// The task of `Link::Sending`: write the frame.
    SendFrame(Frame),
    /// Ask the peer to prove it is alive.
    Ping,
    /// Close the session on the transport.
    Close {
        /// Session being closed.
        session: u32,
    },
    /// All attempts failed.
    GiveUp,
}
