//! `Cx`, the context handed to handlers, entries and exits (spec 4.3).

use crate::effect::Effect;
use crate::ids::{StateId, StatePath};
use crate::runtime::{EventQueues, Runtime};
use crate::sink::Sink;
use crate::spec::Spec;

/// Error recorded while a microstep ran, reported by `send` afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum Fault {
    /// The sink refused an effect.
    EffectOverflow,
    /// The internal queue was full.
    RaiseOverflow,
}

/// Context of a handler: global context, effects, internal events and a
/// read-only view of the active configuration.
pub struct Cx<'step, M: Spec, S> {
    ctx: &'step mut M::Ctx,
    runtime: &'step mut M::Runtime,
    sink: &'step mut S,
    active: Option<&'step StatePath>,
    refreshed: Option<StatePath>,
    fault: Option<Fault>,
}

impl<'step, M: Spec, S: Sink<M::Effect>> Cx<'step, M, S> {
    /// Builds a context; called by `Machine` and by tests of hand-written
    /// `Spec` impls.
    #[doc(hidden)]
    pub fn new(
        ctx: &'step mut M::Ctx,
        runtime: &'step mut M::Runtime,
        sink: &'step mut S,
        active: Option<&'step StatePath>,
    ) -> Self {
        Self {
            ctx,
            runtime,
            sink,
            active,
            refreshed: None,
            fault: None,
        }
    }

    /// The global context.
    #[inline]
    pub fn ctx(&self) -> &M::Ctx {
        self.ctx
    }

    /// The global context, mutably.
    #[inline]
    pub fn ctx_mut(&mut self) -> &mut M::Ctx {
        self.ctx
    }

    /// Emits a user effect.
    #[inline]
    pub fn emit(&mut self, command: M::Effect) {
        self.effect(Effect::User(command));
    }

    /// Raises an internal event, processed after the current microstep and
    /// before deferred events (spec 5.4).
    #[inline]
    pub fn raise(&mut self, event: M::Event) {
        if self.runtime.queues().raise(event).is_err() {
            self.fault = Some(Fault::RaiseOverflow);
        }
    }

    /// Whether `state` is active, including states of sibling regions.
    ///
    /// Reflects the configuration at the start of the current microstep,
    /// updated after each region-internal transition of a parallel state
    /// (see [`refresh_active`](Self::refresh_active)).
    /// Requires `Spec::TRACKS_ACTIVE`; without it, always `false`.
    #[inline]
    pub fn in_state(&self, state: StateId) -> bool {
        let path = self.refreshed.as_ref().or(self.active);
        path.is_some_and(|path| path.contains(state))
    }

    /// Replaces the configuration read by [`in_state`](Self::in_state).
    ///
    /// Generated code calls it after a region-internal transition, so the
    /// next region sees the new configuration (spec 5.6, item 2).
    #[doc(hidden)]
    #[inline]
    pub fn refresh_active(&mut self, path: StatePath) {
        self.refreshed = Some(path);
    }

    /// Pushes any effect (timers, invokes). Called by generated code.
    #[doc(hidden)]
    #[inline]
    pub fn effect(&mut self, effect: Effect<M::Effect>) {
        if self.sink.push(effect).is_err() && self.fault.is_none() {
            self.fault = Some(Fault::EffectOverflow);
        }
    }

    /// The runtime (epochs, history, flags). Called by generated code.
    #[doc(hidden)]
    #[inline]
    pub fn runtime_mut(&mut self) -> &mut M::Runtime {
        self.runtime
    }

    /// The first fault of this microstep.
    #[doc(hidden)]
    #[inline]
    pub fn fault(&self) -> Option<Fault> {
        self.fault
    }
}
