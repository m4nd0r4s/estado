//! Event handlers (`#[on(S)]`), as the user writes them after `#[handlers]`
//! rewrote `goto!` into typed edges (ADR 0001).
//!
//! Leaves bubble every byte they do not expect; the parent of the
//! `RequestLine` and `Headers` substates turns it into `Error`. Handlers
//! that need the data of an ancestor take its fields as a parameter (spec
//! 6.2). `defer!` would be rejected here with E0030 (spec 11, scenario A):
//! a byte stream has no event worth holding back, and a macro error, not a
//! runtime one, is what the spec asks for (milestone M3).

use estado_core::{Next, Sink};

use super::actions::ParserCx;
use super::ctx::{Method, ParseError, RequestSummary};
use super::edges::*;
use super::grammar::*;
use super::model::*;

// --- RequestLine -----------------------------------------------------------

#[inline]
pub fn on_method(fields: &mut RequestLineFields, event: &u8) -> Next<HttpParser, MethodEdges> {
    match *event {
        b'A'..=b'Z' => match push_method_byte(fields.method_code, *event) {
            Some(code) => {
                fields.method_code = code;
                Next::Stay
            }
            None => Next::Goto(MethodEdges::ToError(ParseError::MethodTooLong)),
        },
        b' ' if fields.method_code != 0 => Next::Goto(MethodEdges::ToTarget),
        _ => Next::Bubble,
    }
}

#[inline]
pub fn on_target(fields: &mut RequestLineFields, event: &u8) -> Next<HttpParser, TargetEdges> {
    match *event {
        b' ' if fields.target_length > 0 => Next::Goto(TargetEdges::ToVersion),
        0x21..=0x7e if fields.target_length == MAX_TARGET_LENGTH => {
            Next::Goto(TargetEdges::ToError(ParseError::TargetTooLong))
        }
        0x21..=0x7e => {
            fields.target_length += 1;
            fields.target_hash = mix_target_hash(fields.target_hash, *event);
            Next::Stay
        }
        _ => Next::Bubble,
    }
}

#[inline]
pub fn on_version(
    version: &mut Version,
    fields: &mut RequestLineFields,
    event: &u8,
) -> Next<HttpParser, VersionEdges> {
    match version_step(version.seen, *event) {
        VersionStep::Advance(seen) => {
            version.seen = seen;
            Next::Stay
        }
        VersionStep::Complete => Next::Goto(VersionEdges::ToHeaders(RequestSummary {
            method: Method::from_code(fields.method_code),
            target_length: fields.target_length,
            target_hash: fields.target_hash,
            header_count: 0,
            content_length: 0,
        })),
        VersionStep::Unsupported => {
            Next::Goto(VersionEdges::ToError(ParseError::UnsupportedVersion))
        }
        VersionStep::Malformed => Next::Bubble,
    }
}

/// Parent of `Method`, `Target` and `Version`: a byte they bubbled.
#[inline]
pub fn on_request_line(_event: &u8) -> Next<HttpParser, RequestLineEdges> {
    Next::Goto(RequestLineEdges::ToError(ParseError::BadRequestLine))
}

// --- Headers ---------------------------------------------------------------

#[inline]
pub fn on_line_start(event: &u8) -> Next<HttpParser, LineStartEdges> {
    match *event {
        CR => Next::Goto(LineStartEdges::ToBlockLf),
        byte if is_name_byte(byte) => Next::Goto(LineStartEdges::ToName(Name {
            matched: advance_name_match(0, byte),
        })),
        _ => Next::Bubble,
    }
}

#[inline]
pub fn on_name(name: &mut Name, event: &u8) -> Next<HttpParser, NameEdges> {
    match *event {
        b':' => Next::Goto(NameEdges::ToValue(Value {
            is_content_length: is_content_length(name.matched),
            digits: None,
        })),
        byte if is_name_byte(byte) => {
            name.matched = advance_name_match(name.matched, byte);
            Next::Stay
        }
        _ => Next::Bubble,
    }
}

#[inline]
pub fn on_value(
    value: &mut Value,
    request: &mut RequestSummary,
    event: &u8,
) -> Next<HttpParser, ValueEdges> {
    match *event {
        CR => {
            if value.is_content_length {
                match value.digits {
                    Some(length) => request.content_length = length,
                    None => return Next::Goto(ValueEdges::ToError(ParseError::BadContentLength)),
                }
            }
            if request.header_count >= MAX_HEADERS {
                return Next::Goto(ValueEdges::ToError(ParseError::TooManyHeaders));
            }
            request.header_count += 1;
            Next::Goto(ValueEdges::ToValueLf)
        }
        b' ' | b'\t' => Next::Stay,
        digit @ b'0'..=b'9' if value.is_content_length => match push_digit(value.digits, digit) {
            Some(length) => {
                value.digits = Some(length);
                Next::Stay
            }
            None => Next::Goto(ValueEdges::ToError(ParseError::BadContentLength)),
        },
        0x00..=0x1f | 0x7f => Next::Bubble,
        _ if value.is_content_length => {
            Next::Goto(ValueEdges::ToError(ParseError::BadContentLength))
        }
        _ => Next::Stay,
    }
}

#[inline]
pub fn on_value_lf(event: &u8) -> Next<HttpParser, ValueLfEdges> {
    match *event {
        LF => Next::Goto(ValueLfEdges::ToLineStart),
        _ => Next::Bubble,
    }
}

/// The line feed that ends the headers: the request is complete here when
/// it has no body, otherwise the body starts.
#[inline]
pub fn on_block_lf<S: Sink<()>>(
    request: &mut RequestSummary,
    cx: &mut ParserCx<'_, S>,
    event: &u8,
) -> Next<HttpParser, BlockLfEdges> {
    match *event {
        LF if request.content_length == 0 => {
            cx.ctx_mut().record_completed(request);
            Next::Goto(BlockLfEdges::ToRequestLine)
        }
        LF => Next::Goto(BlockLfEdges::ToBody(Body {
            remaining: request.content_length,
            request: *request,
        })),
        _ => Next::Bubble,
    }
}

/// Parent of the `Headers` substates: a byte they bubbled.
#[inline]
pub fn on_headers(_event: &u8) -> Next<HttpParser, HeadersEdges> {
    Next::Goto(HeadersEdges::ToError(ParseError::BadHeader))
}

// --- Body ------------------------------------------------------------------

#[inline]
pub fn on_body<S: Sink<()>>(
    body: &mut Body,
    cx: &mut ParserCx<'_, S>,
    event: &u8,
) -> Next<HttpParser, BodyEdges> {
    let ctx = cx.ctx_mut();
    ctx.body_checksum = ctx.body_checksum.wrapping_add(u64::from(*event));
    body.remaining -= 1;
    if body.remaining == 0 {
        ctx.record_completed(&body.request);
        Next::Goto(BodyEdges::ToRequestLine)
    } else {
        Next::Stay
    }
}
