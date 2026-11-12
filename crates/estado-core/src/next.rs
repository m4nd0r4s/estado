//! Handler results and typed edges (spec 4.2, ADR 0001).

use core::convert::Infallible;
use core::marker::PhantomData;

use crate::context::Cx;
use crate::ids::StateId;
use crate::sink::Sink;
use crate::slot::Slot;
use crate::spec::Spec;

/// What a handler decided. `Edges` is the edge set of that handler, a type
/// generated per handler; users never build `Goto` themselves.
pub enum Next<M: Spec, Edges: Edge<M> = NoEdge> {
    /// Handled, no transition.
    Stay,
    /// Delegate to the parent state.
    Bubble,
    /// Nobody handles it at this level; apply the unhandled policy.
    Unhandled,
    /// Defer the event (spec 5.5).
    Defer,
    /// Exit and re-enter the handler's own state.
    Reenter,
    /// Transition along one edge of the handler.
    Goto(Edges),
    #[doc(hidden)]
    __Never(PhantomData<fn() -> M>, Infallible),
}

/// A set of edges of one handler: an enum generated per handler, one
/// variant per `goto!`.
pub trait Edge<M: Spec> {
    /// Destinations, one per variant, for the graph.
    const TARGETS: &'static [StateId];

    /// Runs the transition of spec 5.3 for the chosen variant and returns
    /// whether the active configuration changed. Called by generated code.
    #[doc(hidden)]
    fn apply<S: Sink<M::Effect>>(self, slot: &mut Slot<M>, cx: &mut Cx<'_, M, S>) -> bool;
}

/// Edge set of a handler without `goto!`.
pub enum NoEdge {}

impl<M: Spec> Edge<M> for NoEdge {
    const TARGETS: &'static [StateId] = &[];

    fn apply<S: Sink<M::Effect>>(self, _: &mut Slot<M>, _: &mut Cx<'_, M, S>) -> bool {
        match self {}
    }
}
