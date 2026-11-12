//! The HTTP/1 request parser as a `#[machine]` module (ADR 0010).
//!
//! Compared with the hand-written scenario: the compound states hold their
//! shared fields directly (`RequestLine`, `Headers`), the macro generates the
//! `*Fields` views, the dispatch, the edge enums, the entry code and the
//! graph. The byte-level rules are the same functions of
//! [`grammar`](crate::a_http_parser::grammar), so both versions do the same
//! work per byte.

use estado_core::{Cx, Next};

use super::grammar::*;
use super::{Method, ParseError, ParserCtx, RequestSummary};

#[estado_macros::machine(event = u8, ctx = ParserCtx, infallible)]
pub mod parser {
    use super::*;

    /// Root of the machine.
    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum HttpParser {
        /// Compound: reading the request line.
        #[initial(with = new_request_line)]
        RequestLine(RequestLine),
        /// Compound: reading header lines.
        Headers(Headers),
        /// Reading `Content-Length` bytes of body.
        Body(Body),
        /// Final: the input was malformed; the payload is the reason.
        #[terminal]
        Error(ParseError),
    }

    /// `#[initial(with = ..)]`: a request line with everything empty.
    fn new_request_line() -> HttpParser {
        HttpParser::RequestLine(RequestLine {
            method_code: 0,
            target_length: 0,
            target_hash: 0,
            sub: RequestLineSub::Method,
        })
    }

    /// Compound `RequestLine`: what its substates share, and the substates.
    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RequestLine {
        /// Method letters packed one per byte.
        pub method_code: u64,
        /// Target bytes read.
        pub target_length: u32,
        /// Hash of the target bytes.
        pub target_hash: u32,
        /// The active substate.
        #[substates]
        pub sub: RequestLineSub,
    }

    /// Substates of `RequestLine`.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RequestLineSub {
        /// `#[initial]`: upper-case letters up to the first space.
        #[initial]
        Method,
        /// Printable bytes up to the next space.
        Target,
        /// `HTTP/1.x` and the line end.
        Version(Version),
    }

    /// Data of `Version`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct Version {
        /// Bytes of `HTTP/1.x CR LF` read so far.
        pub seen: u8,
    }

    /// Compound `Headers`.
    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Headers {
        /// The request being parsed.
        pub request: RequestSummary,
        /// The active substate.
        #[substates]
        pub sub: HeadersSub,
    }

    /// Substates of `Headers`.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HeadersSub {
        /// `#[initial]`: first byte of a line, a name or the closing CR.
        #[initial]
        LineStart,
        /// Header name up to the colon.
        Name(Name),
        /// Header value up to CR.
        Value(Value),
        /// The line feed after a value.
        ValueLf,
        /// The line feed after the CR that ends the headers.
        BlockLf,
    }

    /// Data of `Name`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Name {
        /// Bytes matching `content-length`, or `NO_MATCH`.
        pub matched: u8,
    }

    /// Data of `Value`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Value {
        /// The header is `Content-Length`: digits are accumulated.
        pub is_content_length: bool,
        /// The number read so far, `None` before the first digit.
        pub digits: Option<u32>,
    }

    /// Data of `Body`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Body {
        /// Body bytes still to read, at least 1.
        pub remaining: u32,
        /// The request this body belongs to.
        pub request: RequestSummary,
    }

    #[handlers]
    impl HttpParser {
        /// Counts the request being started.
        #[entry(RequestLine)]
        fn entry_request_line(cx: &mut Cx<Self>) {
            cx.ctx_mut().requests_started += 1;
        }

        /// Publishes the reason in the context.
        #[entry(Error)]
        fn entry_error(reason: &mut ParseError, cx: &mut Cx<Self>) {
            cx.ctx_mut().error = Some(*reason);
        }

        // --- RequestLine -----------------------------------------------

        #[on(Method)]
        fn on_method(fields: RequestLineFields<'_>, event: &u8) -> Next<Self> {
            match *event {
                b'A'..=b'Z' => match push_method_byte(*fields.method_code, *event) {
                    Some(code) => {
                        *fields.method_code = code;
                        stay!()
                    }
                    None => goto!(Error(ParseError::MethodTooLong)),
                },
                b' ' if *fields.method_code != 0 => goto!(Target),
                _ => bubble!(),
            }
        }

        #[on(Target)]
        fn on_target(fields: RequestLineFields<'_>, event: &u8) -> Next<Self> {
            match *event {
                b' ' if *fields.target_length > 0 => goto!(Version(Version::default())),
                0x21..=0x7e if *fields.target_length == MAX_TARGET_LENGTH => {
                    goto!(Error(ParseError::TargetTooLong))
                }
                0x21..=0x7e => {
                    *fields.target_length += 1;
                    *fields.target_hash = mix_target_hash(*fields.target_hash, *event);
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[on(Version)]
        fn on_version(
            version: &mut Version,
            fields: RequestLineFields<'_>,
            event: &u8,
        ) -> Next<Self> {
            match version_step(version.seen, *event) {
                VersionStep::Advance(seen) => {
                    version.seen = seen;
                    stay!()
                }
                VersionStep::Complete => goto!(Headers {
                    request: RequestSummary {
                        method: Method::from_code(*fields.method_code),
                        target_length: *fields.target_length,
                        target_hash: *fields.target_hash,
                        header_count: 0,
                        content_length: 0,
                    },
                }),
                VersionStep::Unsupported => goto!(Error(ParseError::UnsupportedVersion)),
                VersionStep::Malformed => bubble!(),
            }
        }

        /// Parent of `Method`, `Target` and `Version`: a byte they bubbled.
        #[on(RequestLine)]
        fn on_request_line(_event: &u8) -> Next<Self> {
            goto!(Error(ParseError::BadRequestLine))
        }

        // --- Headers ---------------------------------------------------

        #[on(LineStart)]
        fn on_line_start(event: &u8) -> Next<Self> {
            match *event {
                CR => goto!(BlockLf),
                byte if is_name_byte(byte) => goto!(Name(Name {
                    matched: advance_name_match(0, byte),
                })),
                _ => bubble!(),
            }
        }

        #[on(Name)]
        fn on_name(name: &mut Name, event: &u8) -> Next<Self> {
            match *event {
                b':' => goto!(Value(Value {
                    is_content_length: is_content_length(name.matched),
                    digits: None,
                })),
                byte if is_name_byte(byte) => {
                    name.matched = advance_name_match(name.matched, byte);
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[on(Value)]
        fn on_value(value: &mut Value, headers: HeadersFields<'_>, event: &u8) -> Next<Self> {
            match *event {
                CR => {
                    if value.is_content_length {
                        match value.digits {
                            Some(length) => headers.request.content_length = length,
                            None => return goto!(Error(ParseError::BadContentLength)),
                        }
                    }
                    if headers.request.header_count >= MAX_HEADERS {
                        return goto!(Error(ParseError::TooManyHeaders));
                    }
                    headers.request.header_count += 1;
                    goto!(ValueLf)
                }
                b' ' | b'\t' => stay!(),
                digit @ b'0'..=b'9' if value.is_content_length => {
                    match push_digit(value.digits, digit) {
                        Some(length) => {
                            value.digits = Some(length);
                            stay!()
                        }
                        None => goto!(Error(ParseError::BadContentLength)),
                    }
                }
                0x00..=0x1f | 0x7f => bubble!(),
                _ if value.is_content_length => goto!(Error(ParseError::BadContentLength)),
                _ => stay!(),
            }
        }

        #[on(ValueLf)]
        fn on_value_lf(event: &u8) -> Next<Self> {
            match *event {
                LF => goto!(LineStart),
                _ => bubble!(),
            }
        }

        /// The line feed that ends the headers: the request is complete here
        /// when it has no body, otherwise the body starts.
        #[on(BlockLf)]
        fn on_block_lf(headers: HeadersFields<'_>, cx: &mut Cx<Self>, event: &u8) -> Next<Self> {
            match *event {
                LF if headers.request.content_length == 0 => {
                    cx.ctx_mut().record_completed(headers.request);
                    goto!(RequestLine {
                        method_code: 0,
                        target_length: 0,
                        target_hash: 0,
                    })
                }
                LF => goto!(Body(Body {
                    remaining: headers.request.content_length,
                    request: *headers.request,
                })),
                _ => bubble!(),
            }
        }

        /// Parent of the `Headers` substates: a byte they bubbled.
        #[on(Headers)]
        fn on_headers(_event: &u8) -> Next<Self> {
            goto!(Error(ParseError::BadHeader))
        }

        // --- Body ------------------------------------------------------

        #[on(Body)]
        fn on_body(body: &mut Body, cx: &mut Cx<Self>, event: &u8) -> Next<Self> {
            let ctx = cx.ctx_mut();
            ctx.body_checksum = ctx.body_checksum.wrapping_add(u64::from(*event));
            body.remaining -= 1;
            if body.remaining == 0 {
                ctx.record_completed(&body.request);
                goto!(RequestLine {
                    method_code: 0,
                    target_length: 0,
                    target_hash: 0,
                })
            } else {
                stay!()
            }
        }
    }
}
