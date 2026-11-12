//! States of the parser: the root enum, the compound states with their
//! substates, and the static graph (spec 3.2, 3.5).
//!
//! ```text
//! RequestLine { Method, Target, Version }
//! Headers     { LineStart, Name, Value, ValueLf, BlockLf }
//! Body
//! Error       (#[final], payload = ParseError)
//! ```

use estado_core::{Graph, StateId, StateInfo, StateKind, StatePath};

use super::ctx::{ParseError, RequestSummary};

/// Root of the machine (`#[machine(event = u8, effect = (), infallible)]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpParser {
    /// Compound: reading the request line.
    RequestLine(RequestLine),
    /// Compound: reading header lines.
    Headers(Headers),
    /// Reading `Content-Length` bytes of body.
    Body(Body),
    /// `#[final]`: the input was malformed; the payload is the reason.
    Error(ParseError),
}

/// Compound `RequestLine`: its own fields plus the `#[substates]` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestLine {
    /// What the substates share (the `RequestLineFields` of the macro).
    pub fields: RequestLineFields,
    /// The active substate.
    pub sub: RequestLineSub,
}

/// Fields of `RequestLine` that its substates read and write.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequestLineFields {
    /// Method letters packed one per byte.
    pub method_code: u64,
    /// Target bytes read.
    pub target_length: u32,
    /// Hash of the target bytes.
    pub target_hash: u32,
}

/// Substates of `RequestLine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestLineSub {
    /// `#[initial]`: upper-case letters up to the first space.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headers {
    /// The request being parsed (the `HeadersFields` of the macro).
    pub request: RequestSummary,
    /// The active substate.
    pub sub: HeadersSub,
}

/// Substates of `Headers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadersSub {
    /// `#[initial]`: first byte of a line, a name or the closing CR.
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
    /// Bytes matching `content-length`, or [`NO_MATCH`](super::grammar::NO_MATCH).
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

impl RequestLine {
    /// The value `#[initial]` builds: everything empty, in `Method`.
    #[inline]
    pub fn initial() -> Self {
        Self {
            fields: RequestLineFields::default(),
            sub: RequestLineSub::Method,
        }
    }
}

impl Headers {
    /// Enters `Headers` for `request`, in `LineStart`.
    #[inline]
    pub fn initial(request: RequestSummary) -> Self {
        Self {
            request,
            sub: HeadersSub::LineStart,
        }
    }
}

impl HttpParser {
    /// Id of `RequestLine`.
    pub const REQUEST_LINE: StateId = StateId::new(0);
    /// Id of `Method`.
    pub const METHOD: StateId = StateId::new(1);
    /// Id of `Target`.
    pub const TARGET: StateId = StateId::new(2);
    /// Id of `Version`.
    pub const VERSION: StateId = StateId::new(3);
    /// Id of `Headers`.
    pub const HEADERS: StateId = StateId::new(4);
    /// Id of `LineStart`.
    pub const LINE_START: StateId = StateId::new(5);
    /// Id of `Name`.
    pub const NAME: StateId = StateId::new(6);
    /// Id of `Value`.
    pub const VALUE: StateId = StateId::new(7);
    /// Id of `ValueLf`.
    pub const VALUE_LF: StateId = StateId::new(8);
    /// Id of `BlockLf`.
    pub const BLOCK_LF: StateId = StateId::new(9);
    /// Id of `Body`.
    pub const BODY: StateId = StateId::new(10);
    /// Id of `Error`.
    pub const ERROR: StateId = StateId::new(11);

    /// The active configuration, parent before child.
    pub fn active_path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        match self {
            Self::RequestLine(line) => {
                path.push(Self::REQUEST_LINE);
                path.push(match line.sub {
                    RequestLineSub::Method => Self::METHOD,
                    RequestLineSub::Target => Self::TARGET,
                    RequestLineSub::Version(_) => Self::VERSION,
                });
            }
            Self::Headers(headers) => {
                path.push(Self::HEADERS);
                path.push(match headers.sub {
                    HeadersSub::LineStart => Self::LINE_START,
                    HeadersSub::Name(_) => Self::NAME,
                    HeadersSub::Value(_) => Self::VALUE,
                    HeadersSub::ValueLf => Self::VALUE_LF,
                    HeadersSub::BlockLf => Self::BLOCK_LF,
                });
            }
            Self::Body(_) => path.push(Self::BODY),
            Self::Error(_) => path.push(Self::ERROR),
        }
        path
    }
}

const fn state(
    id: StateId,
    name: &'static str,
    parent: Option<StateId>,
    kind: StateKind,
    depth: u8,
) -> StateInfo {
    StateInfo::new(id, name, parent, kind, depth)
}

static STATES: [StateInfo; 12] = [
    state(
        HttpParser::REQUEST_LINE,
        "RequestLine",
        None,
        StateKind::Compound,
        0,
    )
    .with_initial(HttpParser::METHOD),
    state(
        HttpParser::METHOD,
        "Method",
        Some(HttpParser::REQUEST_LINE),
        StateKind::Atomic,
        1,
    ),
    state(
        HttpParser::TARGET,
        "Target",
        Some(HttpParser::REQUEST_LINE),
        StateKind::Atomic,
        1,
    ),
    state(
        HttpParser::VERSION,
        "Version",
        Some(HttpParser::REQUEST_LINE),
        StateKind::Atomic,
        1,
    ),
    state(HttpParser::HEADERS, "Headers", None, StateKind::Compound, 0)
        .with_initial(HttpParser::LINE_START),
    state(
        HttpParser::LINE_START,
        "LineStart",
        Some(HttpParser::HEADERS),
        StateKind::Atomic,
        1,
    ),
    state(
        HttpParser::NAME,
        "Name",
        Some(HttpParser::HEADERS),
        StateKind::Atomic,
        1,
    ),
    state(
        HttpParser::VALUE,
        "Value",
        Some(HttpParser::HEADERS),
        StateKind::Atomic,
        1,
    ),
    state(
        HttpParser::VALUE_LF,
        "ValueLf",
        Some(HttpParser::HEADERS),
        StateKind::Atomic,
        1,
    ),
    state(
        HttpParser::BLOCK_LF,
        "BlockLf",
        Some(HttpParser::HEADERS),
        StateKind::Atomic,
        1,
    ),
    state(HttpParser::BODY, "Body", None, StateKind::Atomic, 0),
    state(HttpParser::ERROR, "Error", None, StateKind::Final, 0),
];

/// The static graph of the parser. Edges are not listed: the hand-written
/// machines of M2 leave `edges` empty (the macro fills it from
/// [`Edge::TARGETS`](estado_core::Edge::TARGETS)).
pub static GRAPH: Graph = Graph {
    machine: "HttpParser",
    states: &STATES,
    edges: &[],
};
