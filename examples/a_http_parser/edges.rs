//! Edge enums of the handlers and their transition code (spec 5.2, 5.3).
//! One enum per handler, one variant per `goto!` (ADR 0001).
//!
//! No state of the parser has an exit action, and only `RequestLine` and
//! `Error` have entries, so the transitions are a few writes to the slot.
//! Exits would run first, deepest state first, then the slot is taken and
//! written, then the entries run from the child of the LCA down.

use estado_core::{Edge, Sink, Slot, StateId};

use super::actions::{ParserCx, entry_error, entry_request_line};
use super::ctx::{ParseError, RequestSummary};
use super::model::*;

#[inline]
fn request_line_mut(slot: &mut Slot<HttpParser>) -> &mut RequestLine {
    match slot.live_mut() {
        HttpParser::RequestLine(line) => line,
        _ => unreachable!("RequestLine is active"),
    }
}

#[inline]
fn headers_mut(slot: &mut Slot<HttpParser>) -> &mut Headers {
    match slot.live_mut() {
        HttpParser::Headers(headers) => headers,
        _ => unreachable!("Headers is active"),
    }
}

/// Sibling transition inside `RequestLine` (LCA `RequestLine`).
#[inline]
fn move_in_request_line(slot: &mut Slot<HttpParser>, target: RequestLineSub) {
    request_line_mut(slot).sub = target;
}

/// Sibling transition inside `Headers` (LCA `Headers`).
#[inline]
fn move_in_headers(slot: &mut Slot<HttpParser>, target: HeadersSub) {
    headers_mut(slot).sub = target;
}

/// Any state to `Error` (LCA root).
#[inline]
fn leave_to_error<S: Sink<()>>(
    slot: &mut Slot<HttpParser>,
    cx: &mut ParserCx<'_, S>,
    reason: ParseError,
) {
    let _exiting = slot.take();
    slot.put(HttpParser::Error(reason));
    entry_error(reason, cx);
}

/// `Headers` or `Body` to `RequestLine` (LCA root): the next request.
#[inline]
fn leave_to_request_line<S: Sink<()>>(slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) {
    let _exiting = slot.take();
    slot.put(HttpParser::RequestLine(RequestLine::initial()));
    entry_request_line(cx);
}

/// Edges of `#[on(Method)]`.
pub enum MethodEdges {
    /// `goto!(Target)`.
    ToTarget,
    /// `goto!(Error(reason))`.
    ToError(ParseError),
}

impl Edge<HttpParser> for MethodEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::TARGET, HttpParser::ERROR];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToTarget => move_in_request_line(slot, RequestLineSub::Target),
            Self::ToError(reason) => leave_to_error(slot, cx, reason),
        }
        true
    }
}

/// Edges of `#[on(Target)]`.
pub enum TargetEdges {
    /// `goto!(Version)`.
    ToVersion,
    /// `goto!(Error(reason))`.
    ToError(ParseError),
}

impl Edge<HttpParser> for TargetEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::VERSION, HttpParser::ERROR];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToVersion => {
                move_in_request_line(slot, RequestLineSub::Version(Version::default()))
            }
            Self::ToError(reason) => leave_to_error(slot, cx, reason),
        }
        true
    }
}

/// Edges of `#[on(Version)]`.
pub enum VersionEdges {
    /// `goto!(Headers { request })`: LCA root, `Headers` starts in `LineStart`.
    ToHeaders(RequestSummary),
    /// `goto!(Error(reason))`.
    ToError(ParseError),
}

impl Edge<HttpParser> for VersionEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::HEADERS, HttpParser::ERROR];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToHeaders(request) => {
                let _exiting = slot.take();
                slot.put(HttpParser::Headers(Headers::initial(request)));
            }
            Self::ToError(reason) => leave_to_error(slot, cx, reason),
        }
        true
    }
}

/// Edges of `#[on(RequestLine)]`, the parent of the request-line substates.
pub enum RequestLineEdges {
    /// `goto!(Error(reason))`.
    ToError(ParseError),
}

impl Edge<HttpParser> for RequestLineEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::ERROR];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToError(reason) => leave_to_error(slot, cx, reason),
        }
        true
    }
}

/// Edges of `#[on(LineStart)]`.
pub enum LineStartEdges {
    /// `goto!(Name { matched })`.
    ToName(Name),
    /// `goto!(BlockLf)`.
    ToBlockLf,
}

impl Edge<HttpParser> for LineStartEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::NAME, HttpParser::BLOCK_LF];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, _: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToName(name) => move_in_headers(slot, HeadersSub::Name(name)),
            Self::ToBlockLf => move_in_headers(slot, HeadersSub::BlockLf),
        }
        true
    }
}

/// Edges of `#[on(Name)]`.
pub enum NameEdges {
    /// `goto!(Value { .. })`.
    ToValue(Value),
}

impl Edge<HttpParser> for NameEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::VALUE];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, _: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToValue(value) => move_in_headers(slot, HeadersSub::Value(value)),
        }
        true
    }
}

/// Edges of `#[on(Value)]`.
pub enum ValueEdges {
    /// `goto!(ValueLf)`.
    ToValueLf,
    /// `goto!(Error(reason))`.
    ToError(ParseError),
}

impl Edge<HttpParser> for ValueEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::VALUE_LF, HttpParser::ERROR];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToValueLf => move_in_headers(slot, HeadersSub::ValueLf),
            Self::ToError(reason) => leave_to_error(slot, cx, reason),
        }
        true
    }
}

/// Edges of `#[on(ValueLf)]`.
pub enum ValueLfEdges {
    /// `goto!(LineStart)`.
    ToLineStart,
}

impl Edge<HttpParser> for ValueLfEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::LINE_START];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, _: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToLineStart => move_in_headers(slot, HeadersSub::LineStart),
        }
        true
    }
}

/// Edges of `#[on(BlockLf)]`.
pub enum BlockLfEdges {
    /// `goto!(Body { .. })`: LCA root.
    ToBody(Body),
    /// `goto!(RequestLine)`: LCA root, the next request starts.
    ToRequestLine,
}

impl Edge<HttpParser> for BlockLfEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::BODY, HttpParser::REQUEST_LINE];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToBody(body) => {
                let _exiting = slot.take();
                slot.put(HttpParser::Body(body));
            }
            Self::ToRequestLine => leave_to_request_line(slot, cx),
        }
        true
    }
}

/// Edges of `#[on(Headers)]`, the parent of the header substates.
pub enum HeadersEdges {
    /// `goto!(Error(reason))`.
    ToError(ParseError),
}

impl Edge<HttpParser> for HeadersEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::ERROR];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToError(reason) => leave_to_error(slot, cx, reason),
        }
        true
    }
}

/// Edges of `#[on(Body)]`.
pub enum BodyEdges {
    /// `goto!(RequestLine)`.
    ToRequestLine,
}

impl Edge<HttpParser> for BodyEdges {
    const TARGETS: &'static [StateId] = &[HttpParser::REQUEST_LINE];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<HttpParser>, cx: &mut ParserCx<'_, S>) -> bool {
        match self {
            Self::ToRequestLine => leave_to_request_line(slot, cx),
        }
        true
    }
}
