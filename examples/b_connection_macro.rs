//! Scenario B (connection with retry, backoff and a parallel `Connected`
//! state) written with `#[machine]`: the same machine as
//! [`crate::b_connection`], which is the hand-written code the macro must
//! reproduce (milestone M3). Both versions pass the same tests
//! (`tests/b_connection.rs`).
//!
//! The events, commands and the timing constants are shared with the
//! hand-written version; the states, the handlers and everything generated
//! live in the [`machine`] module.

mod machine;

pub use crate::b_connection::{
    BACKOFF_BASE, BACKOFF_CAP, CONNECT_TIMEOUT, Command, DEFER_CAP, DEFER_OVERFLOW_POLICY, Event,
    Frame, MAX_ATTEMPTS, PING_INTERVAL, PONG_TIMEOUT, RAISE_CAP, Session, backoff,
};
pub use machine::conn::{
    BackoffMut, Conn, Connected, ConnectedFields, ConnectingMut, Keepalive, Link, SendingMut,
};
