//! Byte-level rules of the HTTP/1 subset, shared by the estado machine and
//! the hand-written baseline of BM-01, so both do the same work per byte.
//!
//! Accepted request: `METHOD SP target SP HTTP/1.{0,1} CRLF`, then
//! `name: value CRLF` lines, an empty line, and `Content-Length` bytes of
//! body. Everything else is malformed.

/// Carriage return.
pub const CR: u8 = b'\r';
/// Line feed.
pub const LF: u8 = b'\n';
/// Longest accepted target, in bytes.
pub const MAX_TARGET_LENGTH: u32 = 8192;
/// Most headers accepted in one request.
pub const MAX_HEADERS: u16 = 100;
/// Header name whose value is the body length, compared case-insensitively.
pub const CONTENT_LENGTH_NAME: &[u8; 14] = b"content-length";
/// `matched` value of a header name that is not `Content-Length`.
pub const NO_MATCH: u8 = u8::MAX;

/// Whether `byte` may appear in a header name.
#[inline(always)]
pub fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
}

/// Appends a method letter to the packed method (one byte per letter, at
/// most eight). `None` when the method would be longer.
#[inline(always)]
pub fn push_method_byte(code: u64, byte: u8) -> Option<u64> {
    if code >> 56 == 0 {
        Some(code << 8 | u64::from(byte))
    } else {
        None
    }
}

/// Folds one target byte into the running FNV-style hash.
#[inline(always)]
pub fn mix_target_hash(hash: u32, byte: u8) -> u32 {
    (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
}

/// How many bytes of `content-length` the header name matches so far, after
/// reading `byte`; [`NO_MATCH`] once it deviates.
#[inline(always)]
pub fn advance_name_match(matched: u8, byte: u8) -> u8 {
    match CONTENT_LENGTH_NAME.get(usize::from(matched)) {
        Some(expected) if *expected == byte.to_ascii_lowercase() => matched + 1,
        _ => NO_MATCH,
    }
}

/// Whether a header name that matched `matched` bytes is `content-length`.
#[inline(always)]
pub fn is_content_length(matched: u8) -> bool {
    usize::from(matched) == CONTENT_LENGTH_NAME.len()
}

/// Adds one decimal digit to a number being parsed; `None` on overflow.
#[inline(always)]
pub fn push_digit(current: Option<u32>, digit: u8) -> Option<u32> {
    current
        .unwrap_or(0)
        .checked_mul(10)?
        .checked_add(u32::from(digit - b'0'))
}

/// Result of feeding one byte of the version (`HTTP/1.1` CR LF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionStep {
    /// Accepted; the new count of version bytes read.
    Advance(u8),
    /// The final line feed: the request line is complete.
    Complete,
    /// A well-formed `HTTP/` with a version other than 1.0 or 1.1.
    Unsupported,
    /// Any other byte.
    Malformed,
}

/// Bytes of `HTTP/`, then major `1`, `.`, minor `0` or `1`, CR, LF.
#[inline]
pub fn version_step(seen: u8, byte: u8) -> VersionStep {
    match (seen, byte) {
        (0, b'H') | (1 | 2, b'T') | (3, b'P') | (4, b'/') | (6, b'.') | (8, CR) => {
            VersionStep::Advance(seen + 1)
        }
        (5, b'1') => VersionStep::Advance(6),
        (7, b'0' | b'1') => VersionStep::Advance(8),
        (9, LF) => VersionStep::Complete,
        (5 | 7, _) => VersionStep::Unsupported,
        _ => VersionStep::Malformed,
    }
}
