//! The `Spec` impl: nested `match` dispatch from the leaf up (spec 5.1);
//! the chosen edge applies in the same frame.

use estado_core::{Cx, Discard, Edge, Graph, Next, NoRuntime, Outcome, Sink, Slot, Spec};
use estado_core::{StateId, StatePath};

use super::actions::{ParserCx, entry_request_line};
use super::ctx::{ParseError, ParserCtx};
use super::handlers::*;
use super::model::*;

/// What the dispatch does after a handler returned.
enum Resolution {
    Done(Outcome),
    Bubble,
}

#[inline]
fn resolve<E: Edge<HttpParser>, S: Sink<()>>(
    next: Next<HttpParser, E>,
    slot: &mut Slot<HttpParser>,
    cx: &mut ParserCx<'_, S>,
) -> Resolution {
    match next {
        Next::Stay => Resolution::Done(Outcome::Stayed),
        Next::Bubble => Resolution::Bubble,
        Next::Unhandled => Resolution::Done(Outcome::Unhandled(HttpParser::UNHANDLED)),
        Next::Goto(edge) => Resolution::Done(Outcome::Transitioned {
            changed: edge.apply(slot, cx),
        }),
        // No handler of the parser can produce these: `defer!` is E0030 here
        // and nobody calls `reenter!()`.
        Next::Defer | Next::Reenter => unreachable!("the parser never defers or reenters"),
        Next::__Never(_, never) => match never {},
    }
}

/// A byte that bubbled out of a top-level state is unhandled.
#[inline]
fn at_root(resolution: Resolution) -> Outcome {
    match resolution {
        Resolution::Done(outcome) => outcome,
        Resolution::Bubble => Outcome::Unhandled(HttpParser::UNHANDLED),
    }
}

#[inline(always)]
fn dispatch_in_request_line<S: Sink<()>>(
    slot: &mut Slot<HttpParser>,
    cx: &mut ParserCx<'_, S>,
    event: &u8,
) -> Outcome {
    let HttpParser::RequestLine(line) = slot.live_mut() else {
        unreachable!("RequestLine is active")
    };
    let resolution = match &mut line.sub {
        RequestLineSub::Method => resolve(on_method(&mut line.fields, event), slot, cx),
        RequestLineSub::Target => resolve(on_target(&mut line.fields, event), slot, cx),
        RequestLineSub::Version(version) => {
            resolve(on_version(version, &mut line.fields, event), slot, cx)
        }
    };
    match resolution {
        Resolution::Done(outcome) => outcome,
        Resolution::Bubble => at_root(resolve(on_request_line(event), slot, cx)),
    }
}

#[inline(always)]
fn dispatch_in_headers<S: Sink<()>>(
    slot: &mut Slot<HttpParser>,
    cx: &mut ParserCx<'_, S>,
    event: &u8,
) -> Outcome {
    let HttpParser::Headers(headers) = slot.live_mut() else {
        unreachable!("Headers is active")
    };
    let resolution = match &mut headers.sub {
        HeadersSub::LineStart => resolve(on_line_start(event), slot, cx),
        HeadersSub::Name(name) => resolve(on_name(name, event), slot, cx),
        HeadersSub::Value(value) => resolve(on_value(value, &mut headers.request, event), slot, cx),
        HeadersSub::ValueLf => resolve(on_value_lf(event), slot, cx),
        HeadersSub::BlockLf => resolve(on_block_lf(&mut headers.request, cx, event), slot, cx),
    };
    match resolution {
        Resolution::Done(outcome) => outcome,
        Resolution::Bubble => at_root(resolve(on_headers(event), slot, cx)),
    }
}

impl Spec for HttpParser {
    type Event = u8;
    type Effect = ();
    type Ctx = ParserCtx;
    type Output = ParseError;
    type Runtime = NoRuntime<u8>;
    type Effects = Discard;

    const NAME: &'static str = "HttpParser";
    const GRAPH: &'static Graph = &GRAPH;

    fn initial() -> Self {
        Self::RequestLine(RequestLine::initial())
    }

    fn state_id(&self) -> StateId {
        self.active_path().leaf().expect("never empty")
    }

    fn path(&self) -> StatePath {
        self.active_path()
    }

    #[inline]
    fn enter_initial<S: Sink<()>>(&mut self, cx: &mut Cx<'_, Self, S>) {
        entry_request_line(cx);
    }

    // Forced: a byte is too little work to pay for a call per event, and the
    // benchmark (BM-01) moves by 25% with it. See the report of bead 3.9.
    #[inline(always)]
    fn microstep<S: Sink<()>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &u8,
    ) -> Outcome {
        match slot.live_mut() {
            Self::RequestLine(_) => dispatch_in_request_line(slot, cx, event),
            Self::Headers(_) => dispatch_in_headers(slot, cx, event),
            Self::Body(body) => {
                let next = on_body(body, cx, event);
                at_root(resolve(next, slot, cx))
            }
            // `Error` is final: `Machine` ignores events before the microstep.
            Self::Error(_) => Outcome::Unhandled(Self::UNHANDLED),
        }
    }

    fn is_done(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    fn into_output(self) -> Result<ParseError, Self> {
        match self {
            Self::Error(reason) => Ok(reason),
            other => Err(other),
        }
    }
}
