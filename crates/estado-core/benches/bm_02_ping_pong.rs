//! BM-02 (spec 10.3): flat ping-pong of two states, 10^6 events,
//! `effect = ()`, against a hand-written `enum` + `match`.
//!
//! The estado side is written by hand against `Spec` exactly as the macro
//! generates it: one handler per state returning `Next<M, Edges>`, and
//! `Edge::apply` running exit, take, put and entry (ADR 0001, spec 5.3). Both
//! sides do identical work: the exit and entry of a state bump a counter in
//! the context.

use core::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use estado_core::{
    Cx, Discard, Edge, Graph, Machine, Next, NoRuntime, Outcome, Sink, Slot, Spec, StateId,
    StateInfo, StateKind, StatePath, UnhandledPolicy,
};

const EVENTS: u64 = 1_000_000;

/// The only event; every state answers it by moving to the other one. The
/// payload is opaque to the optimizer and keeps the loop from folding.
#[derive(Debug, Clone, Copy)]
enum Event {
    Hit(u32),
}

/// Work done by entries, exits and the hit itself, identical on both sides.
#[derive(Default)]
struct Counters {
    entries: u64,
    exits: u64,
    checksum: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PingPong {
    Ping,
    Pong,
}

// ---------------------------------------------------------------------------
// Hand-written baseline: enum + match.
// ---------------------------------------------------------------------------

impl PingPong {
    #[inline]
    fn step(&mut self, counters: &mut Counters, event: Event) {
        match (*self, event) {
            (Self::Ping, Event::Hit(weight)) => {
                counters.exits += 1;
                counters.checksum += u64::from(weight);
                *self = Self::Pong;
                counters.entries += 1;
            }
            (Self::Pong, Event::Hit(weight)) => {
                counters.exits += 1;
                counters.checksum += u64::from(weight);
                *self = Self::Ping;
                counters.entries += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The same machine through estado, as generated code would write it.
// ---------------------------------------------------------------------------

static STATES: [StateInfo; 2] = [
    StateInfo::new(StateId::new(0), "Ping", None, StateKind::Atomic, 0),
    StateInfo::new(StateId::new(1), "Pong", None, StateKind::Atomic, 0),
];
static GRAPH: Graph = Graph {
    machine: "PingPong",
    states: &STATES,
    edges: &[],
};

type PingPongCx<'step, S> = Cx<'step, PingPong, S>;

impl PingPong {
    const PING_ID: StateId = StateId::new(0);
    const PONG_ID: StateId = StateId::new(1);

    fn id(self) -> StateId {
        match self {
            Self::Ping => Self::PING_ID,
            Self::Pong => Self::PONG_ID,
        }
    }
}

#[inline]
fn exit<S: Sink<()>>(_state: PingPong, cx: &mut PingPongCx<'_, S>) {
    cx.ctx_mut().exits += 1;
}

#[inline]
fn entry<S: Sink<()>>(_state: PingPong, cx: &mut PingPongCx<'_, S>) {
    cx.ctx_mut().entries += 1;
}

/// Edge set of the handler of `Ping`: `goto!(Pong)`.
enum PingEdges {
    ToPong,
}

/// Edge set of the handler of `Pong`: `goto!(Ping)`.
enum PongEdges {
    ToPing,
}

/// The transition of spec 5.3 for a flat machine: exit in place, take,
/// write, enter in place.
#[inline]
fn transition<S: Sink<()>>(
    slot: &mut Slot<PingPong>,
    cx: &mut PingPongCx<'_, S>,
    target: PingPong,
) -> bool {
    let source = *slot.live_mut();
    exit(source, cx);
    let _old = slot.take();
    slot.put(target);
    entry(target, cx);
    source != target
}

impl Edge<PingPong> for PingEdges {
    const TARGETS: &'static [StateId] = &[PingPong::PONG_ID];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<PingPong>, cx: &mut PingPongCx<'_, S>) -> bool {
        match self {
            Self::ToPong => transition(slot, cx, PingPong::Pong),
        }
    }
}

impl Edge<PingPong> for PongEdges {
    const TARGETS: &'static [StateId] = &[PingPong::PING_ID];

    #[inline]
    fn apply<S: Sink<()>>(self, slot: &mut Slot<PingPong>, cx: &mut PingPongCx<'_, S>) -> bool {
        match self {
            Self::ToPing => transition(slot, cx, PingPong::Ping),
        }
    }
}

#[inline]
fn handle_ping<S: Sink<()>>(
    cx: &mut PingPongCx<'_, S>,
    event: &Event,
) -> Next<PingPong, PingEdges> {
    match event {
        Event::Hit(weight) => {
            cx.ctx_mut().checksum += u64::from(*weight);
            Next::Goto(PingEdges::ToPong)
        }
    }
}

#[inline]
fn handle_pong<S: Sink<()>>(
    cx: &mut PingPongCx<'_, S>,
    event: &Event,
) -> Next<PingPong, PongEdges> {
    match event {
        Event::Hit(weight) => {
            cx.ctx_mut().checksum += u64::from(*weight);
            Next::Goto(PongEdges::ToPing)
        }
    }
}

/// Turns a handler decision into the microstep outcome (spec 5.1, 5.3).
#[inline]
fn resolve<Edges: Edge<PingPong>, S: Sink<()>>(
    next: Next<PingPong, Edges>,
    slot: &mut Slot<PingPong>,
    cx: &mut PingPongCx<'_, S>,
) -> Outcome {
    match next {
        Next::Stay => Outcome::Stayed,
        Next::Bubble | Next::Unhandled => Outcome::Unhandled(PingPong::UNHANDLED),
        Next::Defer => Outcome::Deferred,
        Next::Reenter => {
            let state = *slot.live_mut();
            exit(state, cx);
            entry(state, cx);
            Outcome::Transitioned { changed: false }
        }
        Next::Goto(edge) => Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        },
        Next::__Never(_, never) => match never {},
    }
}

impl Spec for PingPong {
    type Event = Event;
    type Effect = ();
    type Ctx = Counters;
    type Output = ();
    type Runtime = NoRuntime<Event>;
    type Effects = Discard;

    const NAME: &'static str = "PingPong";
    const GRAPH: &'static Graph = &GRAPH;
    const UNHANDLED: UnhandledPolicy = UnhandledPolicy::Ignore;

    fn initial() -> Self {
        Self::Ping
    }

    fn state_id(&self) -> StateId {
        self.id()
    }

    fn path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        path.push(self.id());
        path
    }

    #[inline]
    fn enter_initial<S: Sink<()>>(&mut self, cx: &mut PingPongCx<'_, S>) {
        entry(*self, cx);
    }

    #[inline]
    fn microstep<S: Sink<()>>(
        slot: &mut Slot<Self>,
        cx: &mut PingPongCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        match *slot.live_mut() {
            Self::Ping => resolve(handle_ping(cx, event), slot, cx),
            Self::Pong => resolve(handle_pong(cx, event), slot, cx),
        }
    }
}

// ---------------------------------------------------------------------------
// Benchmarks.
// ---------------------------------------------------------------------------

/// One event delivered to the hand-written machine. Not inlined into the
/// loop: events come from outside the machine, and with the whole loop
/// visible the optimizer folds 10^6 toggles into a few instructions.
#[inline(never)]
fn deliver_to_baseline(state: &mut PingPong, counters: &mut Counters, event: Event) {
    state.step(counters, event);
}

/// One event delivered to the estado machine, same boundary as above.
#[inline(never)]
fn deliver_to_estado(machine: &mut Machine<PingPong>, event: Event) {
    // The result is checked once by `assert_sides_agree`, not per event.
    let _ = machine.send_into(event, &mut Discard);
}

fn run_baseline() -> (PingPong, Counters) {
    let mut state = PingPong::Ping;
    let mut counters = Counters::default();
    for weight in 0..EVENTS as u32 {
        deliver_to_baseline(&mut state, &mut counters, Event::Hit(black_box(weight)));
    }
    (black_box(state), black_box(counters))
}

fn run_estado() -> (PingPong, Counters) {
    let mut machine = Machine::<PingPong>::new(Counters::default());
    machine.start_into(&mut Discard).expect("start");
    for weight in 0..EVENTS as u32 {
        deliver_to_estado(&mut machine, Event::Hit(black_box(weight)));
    }
    let state = *machine.state();
    let counters = core::mem::take(machine.ctx_mut());
    (black_box(state), black_box(counters))
}

/// Both sides must compute the same thing, or the comparison is void.
fn assert_sides_agree() {
    let (baseline_state, baseline_counters) = run_baseline();
    let (estado_state, estado_counters) = run_estado();
    assert!(baseline_state == estado_state);
    assert_eq!(baseline_counters.exits, estado_counters.exits);
    assert_eq!(baseline_counters.checksum, estado_counters.checksum);
    // The estado machine also runs the initial entry once.
    assert_eq!(baseline_counters.entries + 1, estado_counters.entries);
}

fn bm_02_ping_pong(criterion: &mut Criterion) {
    assert_sides_agree();
    let mut group = criterion.benchmark_group("bm_02_ping_pong");
    group.throughput(Throughput::Elements(EVENTS));
    group.bench_function("hand_written_enum_match", |bencher| {
        bencher.iter(run_baseline)
    });
    group.bench_function("estado", |bencher| bencher.iter(run_estado));
    group.finish();
}

mod harness {
    #![allow(missing_docs)]
    use super::*;
    criterion_group!(benches, bm_02_ping_pong);
}
use harness::benches;
criterion_main!(benches);
