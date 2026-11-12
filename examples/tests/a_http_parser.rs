//! Scenario A (spec 11): the byte-by-byte HTTP/1 parser, written by hand as
//! the macro would generate it. The same tests must pass for the macro
//! version in milestone M3.

use estado_core::{Discard, IgnoreReason, Machine, Report};
use estado_examples::a_http_parser::baseline::BaselineParser;
use estado_examples::a_http_parser::corpus::{build_requests, corrupt};
use estado_examples::a_http_parser::grammar::{MAX_HEADERS, MAX_TARGET_LENGTH};
use estado_examples::a_http_parser::{HttpParser, Method, ParseError, ParserCtx};

use counting_allocator::allocations_so_far;

#[path = "a_http_parser/counting_allocator.rs"]
mod counting_allocator;

// --- Helpers ---------------------------------------------------------------

fn new_parser() -> Machine<HttpParser> {
    Machine::new(ParserCtx::default())
}

fn feed(parser: &mut Machine<HttpParser>, bytes: &[u8]) -> Report {
    parser
        .send_batch(bytes.iter().copied(), &mut Discard)
        .expect("the parser is infallible")
}

fn parse(bytes: &[u8]) -> Machine<HttpParser> {
    let mut parser = new_parser();
    feed(&mut parser, bytes);
    parser
}

/// Independent copy of the target hash of the parser (FNV-1a, offset 0).
fn target_hash(target: &str) -> u32 {
    target.bytes().fold(0_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    })
}

fn path_after(bytes: &[u8]) -> String {
    parse(bytes).path().to_string()
}

fn error_of(bytes: &[u8]) -> Option<ParseError> {
    let parser = parse(bytes);
    assert_eq!(parser.is_done(), parser.ctx().error.is_some());
    parser.ctx().error
}

const GET: &[u8] = b"GET /index.html HTTP/1.1\r\nHost: example.org\r\n\r\n";
const POST: &[u8] = b"POST /submit?x=1 HTTP/1.0\r\nHost: a\r\nContent-Length: 5\r\n\r\nhello";

// --- Requests --------------------------------------------------------------

#[test]
fn scenario_a_a_simple_get_is_parsed() {
    let parser = parse(GET);
    let ctx = parser.ctx();
    assert_eq!(ctx.requests, 1);
    assert_eq!(ctx.requests_started, 2, "the next request has started");
    assert_eq!(ctx.error, None);
    let request = ctx.recorded[0];
    assert_eq!(request.method, Method::Get);
    assert_eq!(request.target_length, 11);
    assert_eq!(request.target_hash, target_hash("/index.html"));
    assert_eq!(request.header_count, 1);
    assert_eq!(request.content_length, 0);
    assert_eq!(ctx.last, request);
    assert_eq!(parser.path().to_string(), "RequestLine/Method");
}

#[test]
fn scenario_a_headers_are_counted_and_values_may_contain_colons() {
    let request = b"HEAD / HTTP/1.1\r\nHost: a:8080\r\nX-Empty:\r\nAccept:  text/html \r\n\
                    X_Other.Name: 1\r\n\r\n";
    let ctx = *parse(request).ctx();
    assert_eq!(ctx.requests, 1);
    assert_eq!(ctx.last.method, Method::Head);
    assert_eq!(ctx.last.header_count, 4);
    assert_eq!(ctx.header_total, 4);
}

#[test]
fn scenario_a_content_length_is_matched_case_insensitively_and_anywhere() {
    for name in [
        "Content-Length",
        "content-length",
        "CONTENT-LENGTH",
        "cOnTeNt-LeNgTh",
    ] {
        let request = format!("PUT /a HTTP/1.1\r\nHost: x\r\n{name}: 3\r\nAccept: *\r\n\r\nabc");
        let ctx = *parse(request.as_bytes()).ctx();
        assert_eq!(ctx.requests, 1, "{name}");
        assert_eq!(ctx.last.method, Method::Put);
        assert_eq!(ctx.last.content_length, 3, "{name}");
    }
    // Names that only start like it are plain headers.
    let request = b"GET / HTTP/1.1\r\nContent-Length-X: 9\r\nContent-Leng: 9\r\n\r\n";
    let ctx = *parse(request).ctx();
    assert_eq!((ctx.requests, ctx.last.content_length), (1, 0));
}

#[test]
fn scenario_a_a_post_body_is_read_by_content_length() {
    let parser = parse(POST);
    let ctx = parser.ctx();
    assert_eq!(ctx.requests, 1);
    let request = ctx.last;
    assert_eq!(request.method, Method::Post);
    assert_eq!(request.target_length, 11);
    assert_eq!(request.header_count, 2);
    assert_eq!(request.content_length, 5);
    assert_eq!(ctx.body_bytes, 5);
    assert_eq!(
        ctx.body_checksum,
        b"hello".iter().map(|&b| u64::from(b)).sum::<u64>()
    );
}

#[test]
fn scenario_a_a_body_that_looks_like_a_request_is_only_body() {
    let body = "GET /evil HTTP/1.1\r\n\r\n";
    let request = format!(
        "POST /a HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let ctx = *parse(request.as_bytes()).ctx();
    assert_eq!(ctx.requests, 1);
    assert_eq!(ctx.last.content_length as usize, body.len());
}

#[test]
fn scenario_a_pipelined_requests_are_parsed_one_after_the_other() {
    let stream = [GET, POST, GET].concat();
    let mut parser = new_parser();
    let report = feed(&mut parser, &stream);
    assert!(report.handled);
    assert_eq!(report.ignored, None);
    let ctx = parser.ctx();
    assert_eq!(ctx.requests, 3);
    assert_eq!(ctx.requests_started, 4);
    assert_eq!(ctx.recorded[0].method, Method::Get);
    assert_eq!(ctx.recorded[1].method, Method::Post);
    assert_eq!(ctx.recorded[2].method, Method::Get);
    assert_eq!(ctx.recorded[1].content_length, 5);
    assert_eq!(ctx.header_total, 1 + 2 + 1);
    assert_eq!(ctx.body_bytes, 5);
}

#[test]
fn scenario_a_only_the_first_requests_are_recorded() {
    let stream = GET.repeat(12);
    let ctx = *parse(&stream).ctx();
    assert_eq!(ctx.requests, 12);
    assert_eq!(ctx.recorded.len(), 8);
    assert!(
        ctx.recorded
            .iter()
            .all(|request| request.method == Method::Get)
    );
}

#[test]
fn scenario_a_the_active_path_follows_the_bytes() {
    let cases: [(&[u8], &str); 11] = [
        (b"G", "RequestLine/Method"),
        (b"GET ", "RequestLine/Target"),
        (b"GET /a ", "RequestLine/Version"),
        (b"GET /a HTTP/1.1\r", "RequestLine/Version"),
        (b"GET /a HTTP/1.1\r\n", "Headers/LineStart"),
        (b"GET /a HTTP/1.1\r\nHo", "Headers/Name"),
        (b"GET /a HTTP/1.1\r\nHost: x", "Headers/Value"),
        (b"GET /a HTTP/1.1\r\nHost: x\r", "Headers/ValueLf"),
        (b"GET /a HTTP/1.1\r\nHost: x\r\n\r", "Headers/BlockLf"),
        (b"POST /a HTTP/1.1\r\nContent-Length: 2\r\n\r\nx", "Body"),
        (b"GET /a HTTP/1.1\r\n\r\n", "RequestLine/Method"),
    ];
    for (bytes, expected) in cases {
        assert_eq!(
            path_after(bytes),
            expected,
            "{:?}",
            String::from_utf8_lossy(bytes)
        );
    }
}

// --- Chunking --------------------------------------------------------------

#[test]
fn scenario_a_every_split_point_gives_the_same_result() {
    let stream = [GET, POST, GET, b"HEAD / HTTP/1.1\r\n\r\n"].concat();
    let whole = parse(&stream);
    for split in 0..=stream.len() {
        let (head, tail) = stream.split_at(split);
        let mut parser = new_parser();
        feed(&mut parser, head);
        feed(&mut parser, tail);
        assert_eq!(parser.ctx(), whole.ctx(), "split at {split}");
        assert_eq!(parser.path(), whole.path(), "split at {split}");
    }
}

#[test]
fn scenario_a_one_byte_per_send_gives_the_same_result() {
    let stream = [POST, GET].concat();
    let whole = parse(&stream);
    let mut parser = new_parser();
    for &byte in &stream {
        let report = parser.send_into(byte, &mut Discard).expect("infallible");
        assert!(report.handled);
        assert_eq!(report.microsteps, 1);
    }
    assert_eq!(parser.ctx(), whole.ctx());
    assert_eq!(parser.path(), whole.path());
}

#[test]
fn scenario_a_the_batch_report_saturates_instead_of_wrapping() {
    let mut stream = b"GET / HTTP/1.1\r\nX: ".to_vec();
    stream.extend(std::iter::repeat_n(b'v', 70_000));
    let mut parser = new_parser();
    let report = feed(&mut parser, &stream);
    assert_eq!(report.microsteps, u16::MAX);
    assert_eq!(report.transitions, 5);
    assert_eq!(report.unhandled_internal, 0);
}

// --- Malformed input -------------------------------------------------------

#[test]
fn scenario_a_malformed_input_ends_in_error_with_the_reason() {
    let cases: [(&[u8], ParseError); 14] = [
        (b"get / HTTP/1.1\r\n", ParseError::BadRequestLine),
        (b" / HTTP/1.1\r\n", ParseError::BadRequestLine),
        (b"GET  / HTTP/1.1\r\n", ParseError::BadRequestLine),
        (b"GET /a\r\n", ParseError::BadRequestLine),
        (b"GET /a HTTP/1.1\n", ParseError::BadRequestLine),
        (b"GET /a HTTX/1.1\r\n", ParseError::BadRequestLine),
        (b"GET /a HTTP/1.1 \r\n", ParseError::BadRequestLine),
        (b"ABCDEFGHI /", ParseError::MethodTooLong),
        (b"GET /a HTTP/2.0\r\n", ParseError::UnsupportedVersion),
        (b"GET /a HTTP/1.9\r\n", ParseError::UnsupportedVersion),
        (b"GET /a HTTP/1.1\r\nBad Name: x\r\n", ParseError::BadHeader),
        (b"GET /a HTTP/1.1\r\nHost\r\n", ParseError::BadHeader),
        (b"GET /a HTTP/1.1\r\n: x\r\n", ParseError::BadHeader),
        (b"GET /a HTTP/1.1\r\nHost: a\x01\r\n", ParseError::BadHeader),
    ];
    for (bytes, expected) in cases {
        assert_eq!(
            error_of(bytes),
            Some(expected),
            "{:?}",
            String::from_utf8_lossy(bytes)
        );
    }
}

#[test]
fn scenario_a_a_bad_header_terminator_is_an_error() {
    assert_eq!(
        error_of(b"GET / HTTP/1.1\r\nHost: a\rX"),
        Some(ParseError::BadHeader)
    );
    assert_eq!(
        error_of(b"GET / HTTP/1.1\r\nHost: a\r\n\rX"),
        Some(ParseError::BadHeader)
    );
}

#[test]
fn scenario_a_a_bad_content_length_is_an_error() {
    for value in ["", "abc", "12x", "-1", "4294967296", "99999999999999999999"] {
        let request = format!("POST / HTTP/1.1\r\nContent-Length: {value}\r\n\r\n");
        assert_eq!(
            error_of(request.as_bytes()),
            Some(ParseError::BadContentLength),
            "{value:?}"
        );
    }
    let largest = b"POST / HTTP/1.1\r\nContent-Length: 4294967295\r\n\r\n";
    assert_eq!(error_of(largest), None, "u32::MAX is a valid length");
}

#[test]
fn scenario_a_limits_are_enforced() {
    let target = "a".repeat(MAX_TARGET_LENGTH as usize);
    let at_limit = format!("GET /{} HTTP/1.1\r\n\r\n", &target[1..]);
    assert_eq!(error_of(at_limit.as_bytes()), None);
    let over_limit = format!("GET /{target} HTTP/1.1\r\n\r\n");
    assert_eq!(
        error_of(over_limit.as_bytes()),
        Some(ParseError::TargetTooLong)
    );

    let headers = |count: u16| "H: v\r\n".repeat(usize::from(count));
    let at_limit = format!("GET / HTTP/1.1\r\n{}\r\n", headers(MAX_HEADERS));
    assert_eq!(error_of(at_limit.as_bytes()), None);
    let over_limit = format!("GET / HTTP/1.1\r\n{}\r\n", headers(MAX_HEADERS + 1));
    assert_eq!(
        error_of(over_limit.as_bytes()),
        Some(ParseError::TooManyHeaders)
    );
}

#[test]
fn scenario_a_error_is_final_and_later_bytes_are_ignored() {
    let mut parser = parse(b"GET /a HTTP/1.1\r\n\r\nGET /b HTTP/9\r\n");
    assert!(parser.is_done());
    assert_eq!(parser.ctx().requests, 1);
    assert_eq!(parser.path().to_string(), "Error");
    assert_eq!(parser.ctx().error, Some(ParseError::UnsupportedVersion));

    let ctx_before = *parser.ctx();
    let report = feed(&mut parser, GET);
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
    assert!(!report.handled);
    assert_eq!(*parser.ctx(), ctx_before, "ignored bytes change nothing");

    assert_eq!(parser.take_output(), Some(ParseError::UnsupportedVersion));
    assert!(parser.is_done());
    assert_eq!(parser.take_output(), None);
}

#[test]
fn scenario_a_a_running_parser_has_no_output() {
    let mut parser = parse(GET);
    assert!(!parser.is_done());
    assert_eq!(parser.take_output(), None);
    assert_eq!(parser.ctx().requests, 1, "still usable");
}

// --- Equivalence with the hand-written baseline ----------------------------

fn assert_same_as_baseline(bytes: &[u8], label: &str) -> Option<ParseError> {
    let parser = parse(bytes);
    let mut baseline = BaselineParser::new();
    baseline.feed(bytes);
    assert_eq!(parser.ctx(), baseline.ctx(), "{label}");
    assert_eq!(parser.is_done(), baseline.is_failed(), "{label}");
    parser.ctx().error
}

#[test]
fn scenario_a_matches_the_baseline_on_generated_requests() {
    let corpus = build_requests(200_000, 7);
    assert_eq!(assert_same_as_baseline(&corpus.bytes, "corpus"), None);
    let parser = parse(&corpus.bytes);
    assert_eq!(parser.ctx().requests, corpus.requests);
}

#[test]
fn scenario_a_matches_the_baseline_on_corrupted_input() {
    let corpus = build_requests(4_000, 11);
    let mut failures = 0;
    let mut survivors = 0;
    for seed in 0..400 {
        let corrupted = corrupt(&corpus.bytes, 1 + seed as usize % 3, seed);
        match assert_same_as_baseline(&corrupted, &format!("seed {seed}")) {
            Some(_) => failures += 1,
            None => survivors += 1,
        }
    }
    assert!(
        failures > 50,
        "corruption must reach the error paths: {failures}"
    );
    assert!(survivors > 50, "and some inputs must survive: {survivors}");
}

// --- Allocation ------------------------------------------------------------

#[test]
fn scenario_a_the_hot_path_does_not_allocate() {
    let corpus = build_requests(300_000, 3);
    let mut parser = new_parser();
    // The first byte starts the machine; everything after is the hot path.
    feed(&mut parser, &corpus.bytes[..1]);
    let before = allocations_so_far();
    let report = feed(&mut parser, &corpus.bytes[1..]);
    let after = allocations_so_far();
    assert!(report.handled);
    assert_eq!(parser.ctx().requests, corpus.requests);
    assert_eq!(after - before, 0, "send_batch allocated");
}
