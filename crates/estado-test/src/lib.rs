//! Test harness for `estado` machines: a virtual clock and an in-memory
//! [`Driver`](estado_core::Driver) (spec 10.1).
//!
//! ```ignore
//! let mut harness = Harness::new(Machine::<Conn>::new(env));
//! harness.send(Event::Connect).expect_effect(|command| matches!(command, Command::Dial(_)));
//! harness.advance(13.secs());
//! assert_state!(harness, Conn::Backoff(_));
//! harness.complete_invoke(|task| matches!(task, Command::FetchUser(_)), Event::Fetched(user));
//! harness.assert_no_pending_timers();
//! ```

mod clock;
mod harness;
mod pending;

pub use clock::DurationExt;
pub use harness::{Harness, StepView};
pub use pending::{PendingInvoke, PendingTimer};

/// Asserts that the harness machine's root state matches a pattern.
///
/// ```ignore
/// assert_state!(harness, Conn::Backoff(_));
/// ```
#[macro_export]
macro_rules! assert_state {
    ($harness:expr, $pattern:pat $(if $guard:expr)?) => {{
        let state = $harness.machine().state();
        assert!(
            matches!(state, $pattern $(if $guard)?),
            "state {:?} does not match `{}`",
            $harness.machine().path(),
            stringify!($pattern)
        );
    }};
}
