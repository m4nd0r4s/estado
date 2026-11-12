//! Scenario B: a connection with retry, exponential backoff and a parallel
//! `Connected` state, written by hand against `estado-core` exactly as
//! `#[machine]` would generate it.
//!
//! ```text
//! Disconnected (#[initial])
//! Connecting { attempt }   after 3 s => ConnectTimeout, invoke Dial
//! Backoff { attempt }      after 100 ms * 2^attempt (capped at 1 s) => Retry
//! Connected (parallel, Session)
//!   link:      Idle (#[initial]), Sending { frame } (invoke SendFrame)
//!   keepalive: Pinging (#[initial], after 5 s => PingDue),
//!              AwaitingPong (after 2 s => PongTimeout, entry emits Ping),
//!              Dead (entry raises KeepaliveDead, handler leaves Connected)
//! ```
//!
//! What it stresses (spec 11, B):
//!
//! - **epochs**: every state with a timer or an invoke gets a new epoch on
//!   each entry, so a late timeout or dial result of an earlier instance is
//!   `Stale`, even when the runtime did not cancel it in time (7.1, 7.2);
//! - **regions**: `link` and `keepalive` run side by side, a region handler
//!   reads its sibling with `cx.in_state`, and `Dead` leaves all regions in
//!   reverse declaration order (5.3, 5.6);
//! - **defer**: `Send` is deferred while `Connecting`, `Backoff`, while a
//!   frame is in flight and while the peer owes a pong; it is redelivered, in
//!   order, whenever the configuration changes (5.5). The queue holds
//!   [`DEFER_CAP`] events and a full queue follows
//!   [`DEFER_OVERFLOW_POLICY`] (`error`).
//!
//! Open item 13.4 (regions: sequential or all-see-event) is discussed at the
//! top of the `dispatch` module.
//!
//! Module map, one file per kind of generated code:
//!
//! | File | What the macro would generate there |
//! | --- | --- |
//! | `model` | the user's own types: states, events, commands, constants |
//! | `views` | `ConnectedFields` and the `*Mut` views |
//! | `graph` | state ids, `Graph`, `path()` |
//! | `runtime` | queues and epoch counters |
//! | `tasks` | timer and invoke keys, start and cancel per state |
//! | `actions` | entries and exits, linear code per level |
//! | `edges` | one edge enum per handler, with the transition code of 5.3 |
//! | `handlers` | the user's handlers, after `goto!` was rewritten |
//! | `dispatch` | decision phase, the regions of `Connected` |
//! | `spec` | the `Spec` impl, `timer_event`, `accepts_invoke` |

mod actions;
mod dispatch;
mod edges;
mod graph;
mod handlers;
mod model;
mod runtime;
mod spec;
mod tasks;
mod views;

pub use model::{
    BACKOFF_BASE, BACKOFF_CAP, CONNECT_TIMEOUT, Command, Conn, Connected, DEFER_CAP,
    DEFER_OVERFLOW_POLICY, Event, Frame, Keepalive, Link, MAX_ATTEMPTS, PING_INTERVAL,
    PONG_TIMEOUT, RAISE_CAP, Session, backoff,
};
pub use runtime::ConnRuntime;
pub use views::{BackoffMut, ConnectedFields, ConnectingMut, SendingMut};
