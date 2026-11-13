//! BM-01 (spec 10.3): the HTTP/1 parser of scenario A through estado against
//! an equivalent hand-written parser, on about 1 MB of requests.
//!
//! Both sides parse the same bytes, do the same work per byte and fill the
//! same `ParserCtx`; `assert_sides_agree` checks that once. The estado side
//! is the machine of `a_http_parser`, fed with `send_batch`. The bytes come
//! from `black_box`, so the optimizer cannot fold the parse away.
//!
//! Status (Apple M-series, release): the target of 5% is not met. The
//! baseline takes about 1.0 ms per MB, the hand-written machine of
//! `a_http_parser` about 1.6 ms with `send_into` per byte and with
//! `send_batch`. See ADR 0017 and spec errata 14 for the breakdown and the
//! revised target; `estado_send_into` is kept to show what `send_batch` adds
//! on top. The `send_batch` report goes through `black_box` like the
//! context: a caller reads it, so it is work the machine must do.
//!
//! M3 adds the `estado_macro_*` functions: the same parser written with
//! `#[machine]` (`a_http_parser_macro`), the machine ADR 0009 asks to
//! re-measure. Its edges write in place below the root and `microstep` is
//! `#[inline(always)]`.

use core::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use estado_core::{Cx, Discard, Machine, Runtime, Slot, Spec};
use estado_examples::a_http_parser::baseline::BaselineParser;
use estado_examples::a_http_parser::corpus::{Corpus, build_requests};
use estado_examples::a_http_parser::{HttpParser, ParserCtx};
use estado_examples::a_http_parser_macro::HttpParser as MacroParser;

const CORPUS_BYTES: usize = 1_000_000;
const SEED: u64 = 0x5eed;

fn run_baseline(bytes: &[u8]) -> ParserCtx {
    let mut parser = BaselineParser::new();
    parser.feed(black_box(bytes));
    black_box(*parser.ctx())
}

fn run_estado(bytes: &[u8]) -> ParserCtx {
    let mut machine = Machine::<HttpParser>::new(ParserCtx::default());
    // The result is checked once by `assert_sides_agree`, not per run.
    let _ = black_box(machine.send_batch(black_box(bytes).iter().copied(), &mut Discard));
    black_box(*machine.ctx())
}

/// Same, one `send_into` per byte: shows what `send_batch` adds on top.
fn run_estado_send_into(bytes: &[u8]) -> ParserCtx {
    let mut machine = Machine::<HttpParser>::new(ParserCtx::default());
    for &byte in black_box(bytes) {
        let _ = machine.send_into(byte, &mut Discard);
    }
    black_box(*machine.ctx())
}

/// The machine written with `#[machine]`, fed with `send_batch`.
fn run_macro(bytes: &[u8]) -> ParserCtx {
    let mut machine = Machine::<MacroParser>::new(ParserCtx::default());
    let _ = black_box(machine.send_batch(black_box(bytes).iter().copied(), &mut Discard));
    black_box(*machine.ctx())
}

/// Same, one `send_into` per byte.
fn run_macro_send_into(bytes: &[u8]) -> ParserCtx {
    let mut machine = Machine::<MacroParser>::new(ParserCtx::default());
    for &byte in black_box(bytes) {
        let _ = machine.send_into(byte, &mut Discard);
    }
    black_box(*machine.ctx())
}

/// The generated `microstep` called directly, without `Machine`: no poison
/// guard, no queues, no report. This is the generated code on its own, which
/// is what ADR 0009 calls "the shape of generated code"; the rest of the
/// ratio is the machine layer.
fn run_microstep<M: Spec<Event = u8, Effect = (), Ctx = ParserCtx>>(bytes: &[u8]) -> ParserCtx {
    let mut ctx = ParserCtx::default();
    let mut runtime = <M::Runtime as Runtime>::NEW;
    let mut slot = Slot::Live(M::initial());
    let mut sink = Discard;
    {
        let mut cx = Cx::<M, Discard>::new(&mut ctx, &mut runtime, &mut sink, None);
        M::enter_initial(slot.live_mut(), &mut cx);
    }
    for &byte in black_box(bytes) {
        let mut cx = Cx::<M, Discard>::new(&mut ctx, &mut runtime, &mut sink, None);
        let _ = M::microstep(&mut slot, &mut cx, &byte);
    }
    black_box(ctx)
}

/// Both sides must compute the same thing, or the comparison is void.
fn assert_sides_agree(corpus: &Corpus) {
    let baseline = run_baseline(&corpus.bytes);
    let estado = run_estado(&corpus.bytes);
    assert_eq!(baseline, estado);
    assert_eq!(estado, run_estado_send_into(&corpus.bytes));
    assert_eq!(estado, run_macro(&corpus.bytes));
    assert_eq!(estado, run_macro_send_into(&corpus.bytes));
    assert_eq!(estado, run_microstep::<HttpParser>(&corpus.bytes));
    assert_eq!(estado, run_microstep::<MacroParser>(&corpus.bytes));
    assert_eq!(estado.requests, corpus.requests);
    assert_eq!(estado.error, None);
}

fn bm_01_http_parser(criterion: &mut Criterion) {
    let corpus = build_requests(CORPUS_BYTES, SEED);
    assert_sides_agree(&corpus);
    let mut group = criterion.benchmark_group("bm_01_http_parser");
    group.throughput(Throughput::Bytes(corpus.bytes.len() as u64));
    group.bench_function("hand_written_enum_match", |bencher| {
        bencher.iter(|| run_baseline(&corpus.bytes))
    });
    group.bench_function("estado_send_batch", |bencher| {
        bencher.iter(|| run_estado(&corpus.bytes))
    });
    group.bench_function("estado_send_into", |bencher| {
        bencher.iter(|| run_estado_send_into(&corpus.bytes))
    });
    group.bench_function("estado_macro_send_batch", |bencher| {
        bencher.iter(|| run_macro(&corpus.bytes))
    });
    group.bench_function("estado_macro_send_into", |bencher| {
        bencher.iter(|| run_macro_send_into(&corpus.bytes))
    });
    group.bench_function("estado_microstep_direct", |bencher| {
        bencher.iter(|| run_microstep::<HttpParser>(&corpus.bytes))
    });
    group.bench_function("estado_macro_microstep_direct", |bencher| {
        bencher.iter(|| run_microstep::<MacroParser>(&corpus.bytes))
    });
    group.finish();
}

mod harness {
    #![allow(missing_docs)]
    use super::*;
    criterion_group!(benches, bm_01_http_parser);
}
use harness::benches;
criterion_main!(benches);
