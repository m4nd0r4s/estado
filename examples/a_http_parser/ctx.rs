//! What the parser observes: the context shared by the estado machine and
//! the baseline. Everything is `Copy` and fixed size, so the hot path never
//! allocates.

/// Why the input was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// The request line is malformed (bad method, empty target, bad bytes,
    /// missing CRLF).
    BadRequestLine,
    /// The method has more than eight letters.
    MethodTooLong,
    /// The target is longer than [`MAX_TARGET_LENGTH`](super::grammar::MAX_TARGET_LENGTH).
    TargetTooLong,
    /// `HTTP/` followed by a version other than 1.0 or 1.1.
    UnsupportedVersion,
    /// A header line is malformed (bad name, missing colon, control byte).
    BadHeader,
    /// More than [`MAX_HEADERS`](super::grammar::MAX_HEADERS) headers.
    TooManyHeaders,
    /// `Content-Length` is empty, not a number, or does not fit in `u32`.
    BadContentLength,
}

/// Request method, as far as the parser recognizes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `DELETE`.
    Delete,
    /// `HEAD`.
    Head,
    /// Any other well-formed method.
    #[default]
    Other,
}

const fn pack(name: &[u8]) -> u64 {
    let mut code = 0_u64;
    let mut index = 0;
    while index < name.len() {
        code = code << 8 | name[index] as u64;
        index += 1;
    }
    code
}

impl Method {
    /// The method of a packed method code (one byte per letter).
    #[inline]
    pub fn from_code(code: u64) -> Self {
        const GET: u64 = pack(b"GET");
        const POST: u64 = pack(b"POST");
        const PUT: u64 = pack(b"PUT");
        const DELETE: u64 = pack(b"DELETE");
        const HEAD: u64 = pack(b"HEAD");
        match code {
            GET => Self::Get,
            POST => Self::Post,
            PUT => Self::Put,
            DELETE => Self::Delete,
            HEAD => Self::Head,
            _ => Self::Other,
        }
    }
}

/// What the parser kept of one request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequestSummary {
    /// The method.
    pub method: Method,
    /// Length of the target in bytes.
    pub target_length: u32,
    /// FNV-style hash of the target bytes.
    pub target_hash: u32,
    /// Number of headers.
    pub header_count: u16,
    /// Value of `Content-Length`, 0 when absent.
    pub content_length: u32,
}

/// How many completed requests keep their [`RequestSummary`] in the context.
pub const RECORDED_REQUESTS: usize = 8;

/// Global context of the parser (`ctx = ParserCtx`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParserCtx {
    /// Requests the parser started: bumped by the entry of `RequestLine`.
    pub requests_started: u32,
    /// Requests parsed completely, body included.
    pub requests: u32,
    /// Summaries of the first [`RECORDED_REQUESTS`] completed requests.
    pub recorded: [RequestSummary; RECORDED_REQUESTS],
    /// The last completed request.
    pub last: RequestSummary,
    /// Headers of all completed requests.
    pub header_total: u64,
    /// Declared body bytes of all completed requests.
    pub body_bytes: u64,
    /// Wrapping sum of every body byte read.
    pub body_checksum: u64,
    /// Set by the entry of `Error`.
    pub error: Option<ParseError>,
}

impl ParserCtx {
    /// Books a finished request.
    #[inline]
    pub fn record_completed(&mut self, request: &RequestSummary) {
        if let Some(entry) = self.recorded.get_mut(self.requests as usize) {
            *entry = *request;
        }
        self.last = *request;
        self.requests += 1;
        self.header_total += u64::from(request.header_count);
        self.body_bytes += u64::from(request.content_length);
    }
}
