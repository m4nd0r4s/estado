//! The global context: one pointer wide, and free of allocation unless a
//! test asks for a trace.
//!
//! Scenarios A to C keep their trace in a `Vec<String>`. This one is meant to
//! be reused by BM-04, 10,000 machines times 60 events, so the trace is an
//! `Option<Box<..>>` that stays `None` there: `note` is one branch and nothing
//! else. The lines are `&'static str`, so even a traced run only allocates when
//! the log vector grows.

/// The log of a traced run.
#[derive(Debug, Default)]
struct TraceLog(Vec<&'static str>);

/// Global context of `Character` (`#[machine(ctx = GameCtx)]`).
#[derive(Debug, Default)]
pub struct GameCtx {
    trace: Option<Box<TraceLog>>,
}

impl GameCtx {
    /// No trace: what a game or a benchmark uses.
    pub const fn untraced() -> Self {
        Self { trace: None }
    }

    /// Records every handler, entry and exit, for tests.
    pub fn traced() -> Self {
        Self {
            trace: Some(Box::default()),
        }
    }

    /// Appends a line to the trace, when there is one.
    #[inline]
    pub fn note(&mut self, line: &'static str) {
        if let Some(log) = self.trace.as_deref_mut() {
            log.0.push(line);
        }
    }

    /// Takes the lines recorded so far (empty when untraced).
    pub fn take_trace(&mut self) -> Vec<&'static str> {
        self.trace
            .as_deref_mut()
            .map(|log| std::mem::take(&mut log.0))
            .unwrap_or_default()
    }
}
