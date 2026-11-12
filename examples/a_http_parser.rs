//! Scenario A (spec 11): a byte-by-byte HTTP/1 request parser, written by
//! hand exactly as `#[machine(event = u8, effect = (), infallible)]` would
//! generate it (milestone M2), and the baseline of BM-01.
//!
//! The request line, the headers and the body are compound or atomic states
//! of [`HttpParser`]; the parser accepts the next request (pipelining) until
//! the input is malformed, which ends it in the root `#[final]` state
//! `Error`, whose payload is the [`ParseError`]. Events are plain `u8`, so
//! `Machine::send_batch(bytes.iter().copied(), &mut Discard)` feeds a
//! buffer.
//!
//! ```
//! use estado_core::{Discard, Machine};
//! use estado_examples::a_http_parser::{HttpParser, Method, ParserCtx};
//!
//! let mut parser = Machine::<HttpParser>::new(ParserCtx::default());
//! let request = b"GET /index.html HTTP/1.1\r\nHost: example.org\r\n\r\n";
//! parser.send_batch(request.iter().copied(), &mut Discard).unwrap();
//! assert_eq!(parser.ctx().requests, 1);
//! assert_eq!(parser.ctx().last.method, Method::Get);
//! assert_eq!(parser.path().to_string(), "RequestLine/Method");
//! ```

mod actions;
pub mod baseline;
pub mod corpus;
mod ctx;
mod dispatch;
mod edges;
pub mod grammar;
mod handlers;
mod model;

pub use ctx::{Method, ParseError, ParserCtx, RECORDED_REQUESTS, RequestSummary};
pub use model::{
    Body, GRAPH, Headers, HeadersSub, HttpParser, Name, RequestLine, RequestLineFields,
    RequestLineSub, Value, Version,
};
