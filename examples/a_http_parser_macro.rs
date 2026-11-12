//! Scenario A (the byte-by-byte HTTP/1 parser) written with `#[machine]`: the
//! same machine as [`crate::a_http_parser`], which is the hand-written code
//! the macro must reproduce (milestone M3), and the machine BM-01 measures
//! against the hand-written baseline. Both versions pass the same tests
//! (`tests/a_http_parser.rs`).
//!
//! The context, the grammar, the corpus and the baseline are shared with the
//! hand-written version; the states, the handlers and everything generated
//! live in the [`machine`] module.

mod machine;

pub use crate::a_http_parser::{
    Method, ParseError, ParserCtx, RECORDED_REQUESTS, RequestSummary, baseline, corpus, grammar,
};
pub use machine::parser::{
    Body, Headers, HeadersFields, HeadersSub, HttpParser, Name, RequestLine, RequestLineFields,
    RequestLineSub, Value, Version,
};
