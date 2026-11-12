//! Entry actions (`#[entry(S)]`). The parser has no exit actions.

use estado_core::{Cx, Sink};

use super::ctx::ParseError;
use super::model::HttpParser;

/// The `Cx` of the parser.
pub type ParserCx<'step, S> = Cx<'step, HttpParser, S>;

/// `#[entry(RequestLine)]`: counts the request being started.
#[inline]
pub fn entry_request_line<S: Sink<()>>(cx: &mut ParserCx<'_, S>) {
    cx.ctx_mut().requests_started += 1;
}

/// `#[entry(Error)]`: publishes the reason in the context.
#[inline]
pub fn entry_error<S: Sink<()>>(reason: ParseError, cx: &mut ParserCx<'_, S>) {
    cx.ctx_mut().error = Some(reason);
}
