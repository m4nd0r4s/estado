//! The hand-written side of BM-01: the same parser as a plain enum and one
//! `match` per byte, producing the same [`ParserCtx`].
//!
//! It shares only the byte-level rules of [`grammar`](super::grammar) with
//! the estado machine; states, transitions, entry hooks and bookkeeping are
//! written out flat, the way one would without a library.

use super::ctx::{Method, ParseError, ParserCtx, RequestSummary};
use super::grammar::*;

#[derive(Debug, Clone, Copy)]
enum State {
    Method {
        code: u64,
    },
    Target {
        code: u64,
        length: u32,
        hash: u32,
    },
    Version {
        code: u64,
        length: u32,
        hash: u32,
        seen: u8,
    },
    LineStart {
        request: RequestSummary,
    },
    Name {
        request: RequestSummary,
        matched: u8,
    },
    Value {
        request: RequestSummary,
        is_content_length: bool,
        digits: Option<u32>,
    },
    ValueLf {
        request: RequestSummary,
    },
    BlockLf {
        request: RequestSummary,
    },
    Body {
        remaining: u32,
        request: RequestSummary,
    },
    Failed,
}

/// A byte-by-byte HTTP/1 parser without any state machine library.
#[derive(Debug)]
pub struct BaselineParser {
    state: State,
    ctx: ParserCtx,
}

impl Default for BaselineParser {
    fn default() -> Self {
        Self::new()
    }
}

impl BaselineParser {
    /// A parser at the start of its first request.
    pub fn new() -> Self {
        Self {
            state: State::Method { code: 0 },
            ctx: ParserCtx {
                requests_started: 1,
                ..ParserCtx::default()
            },
        }
    }

    /// The observable results, like the context of the estado machine.
    pub fn ctx(&self) -> &ParserCtx {
        &self.ctx
    }

    /// Whether the input was malformed (the estado `Error` state).
    pub fn is_failed(&self) -> bool {
        matches!(self.state, State::Failed)
    }

    /// Feeds bytes; after a failure they are ignored.
    #[inline]
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.step(byte);
        }
    }

    #[inline]
    fn fail(&mut self, reason: ParseError) {
        self.state = State::Failed;
        self.ctx.error = Some(reason);
    }

    #[inline]
    fn start_next_request(&mut self) {
        self.state = State::Method { code: 0 };
        self.ctx.requests_started += 1;
    }

    #[inline]
    fn step(&mut self, byte: u8) {
        match &mut self.state {
            State::Method { code } => match byte {
                b'A'..=b'Z' => match push_method_byte(*code, byte) {
                    Some(packed) => *code = packed,
                    None => self.fail(ParseError::MethodTooLong),
                },
                b' ' if *code != 0 => {
                    self.state = State::Target {
                        code: *code,
                        length: 0,
                        hash: 0,
                    };
                }
                _ => self.fail(ParseError::BadRequestLine),
            },
            State::Target { code, length, hash } => match byte {
                b' ' if *length > 0 => {
                    self.state = State::Version {
                        code: *code,
                        length: *length,
                        hash: *hash,
                        seen: 0,
                    };
                }
                0x21..=0x7e if *length == MAX_TARGET_LENGTH => {
                    self.fail(ParseError::TargetTooLong);
                }
                0x21..=0x7e => {
                    *length += 1;
                    *hash = mix_target_hash(*hash, byte);
                }
                _ => self.fail(ParseError::BadRequestLine),
            },
            State::Version {
                code,
                length,
                hash,
                seen,
            } => match version_step(*seen, byte) {
                VersionStep::Advance(next) => *seen = next,
                VersionStep::Complete => {
                    let request = RequestSummary {
                        method: Method::from_code(*code),
                        target_length: *length,
                        target_hash: *hash,
                        header_count: 0,
                        content_length: 0,
                    };
                    self.state = State::LineStart { request };
                }
                VersionStep::Unsupported => self.fail(ParseError::UnsupportedVersion),
                VersionStep::Malformed => self.fail(ParseError::BadRequestLine),
            },
            State::LineStart { request } => match byte {
                CR => self.state = State::BlockLf { request: *request },
                _ if is_name_byte(byte) => {
                    self.state = State::Name {
                        request: *request,
                        matched: advance_name_match(0, byte),
                    };
                }
                _ => self.fail(ParseError::BadHeader),
            },
            State::Name { request, matched } => match byte {
                b':' => {
                    self.state = State::Value {
                        request: *request,
                        is_content_length: is_content_length(*matched),
                        digits: None,
                    };
                }
                _ if is_name_byte(byte) => *matched = advance_name_match(*matched, byte),
                _ => self.fail(ParseError::BadHeader),
            },
            State::Value {
                request,
                is_content_length,
                digits,
            } => match byte {
                CR => {
                    if *is_content_length {
                        match *digits {
                            Some(length) => request.content_length = length,
                            None => return self.fail(ParseError::BadContentLength),
                        }
                    }
                    if request.header_count >= MAX_HEADERS {
                        return self.fail(ParseError::TooManyHeaders);
                    }
                    request.header_count += 1;
                    self.state = State::ValueLf { request: *request };
                }
                b' ' | b'\t' => {}
                b'0'..=b'9' if *is_content_length => match push_digit(*digits, byte) {
                    Some(length) => *digits = Some(length),
                    None => self.fail(ParseError::BadContentLength),
                },
                0x00..=0x1f | 0x7f => self.fail(ParseError::BadHeader),
                _ if *is_content_length => self.fail(ParseError::BadContentLength),
                _ => {}
            },
            State::ValueLf { request } => match byte {
                LF => self.state = State::LineStart { request: *request },
                _ => self.fail(ParseError::BadHeader),
            },
            State::BlockLf { request } => match byte {
                LF if request.content_length == 0 => {
                    self.ctx.record_completed(request);
                    self.start_next_request();
                }
                LF => {
                    self.state = State::Body {
                        remaining: request.content_length,
                        request: *request,
                    };
                }
                _ => self.fail(ParseError::BadHeader),
            },
            State::Body { remaining, request } => {
                self.ctx.body_checksum = self.ctx.body_checksum.wrapping_add(u64::from(byte));
                *remaining -= 1;
                if *remaining == 0 {
                    self.ctx.record_completed(request);
                    self.start_next_request();
                }
            }
            State::Failed => {}
        }
    }
}
