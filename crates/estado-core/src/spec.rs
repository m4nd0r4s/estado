//! The `Spec` trait, implemented by `#[machine]` or by hand (spec 4.1).

use crate::context::Cx;
use crate::effect::{InvokeKey, TimerKey};
use crate::graph::Graph;
use crate::ids::{StateId, StatePath};
use crate::report::Outcome;
use crate::runtime::Runtime;
use crate::sink::Sink;
use crate::slot::Slot;

/// What happens to an event nobody handles (spec 5.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnhandledPolicy {
    /// External: `Ok` with `ignored = Unhandled`. Internal: counted.
    Ignore,
    /// External: `Err(Unhandled(event))`. Internal: counted.
    Error,
    /// Panic (the machine is poisoned).
    Panic,
}

/// What happens when the defer queue is full (spec 5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferOverflowPolicy {
    /// `Err(DeferOverflow(event))`: state unchanged when the external event
    /// overflowed, machine poisoned when a raised or redelivered one did.
    Error,
    /// Drop the oldest deferred event to make room.
    DropOldest,
}

/// A state machine definition: its types, graph and generated step code.
///
/// The macro implements it; every generated item is writable by hand
/// against this trait (spec principle 6). See the crate examples.
pub trait Spec: Sized {
    /// Events the machine reacts to.
    type Event;
    /// User effects (commands) the machine emits.
    type Effect;
    /// Global context, `()` when not declared.
    type Ctx;
    /// Payload of the root `#[final]` state, or `()`.
    type Output;
    /// Auxiliary state: queues, epochs, history. Zero-sized when unused.
    type Runtime: Runtime<Event = Self::Event>;
    /// Effect buffer of [`Machine::send`](crate::Machine::send), usually
    /// [`InlineEffects`](crate::InlineEffects) with `fx_inline` capacity.
    type Effects: Sink<Self::Effect> + Default;

    /// Machine name.
    const NAME: &'static str;
    /// Static graph.
    const GRAPH: &'static Graph;
    /// Default unhandled policy.
    const UNHANDLED: UnhandledPolicy = UnhandledPolicy::Ignore;
    /// Defer overflow policy.
    const DEFER_OVERFLOW: DeferOverflowPolicy = DeferOverflowPolicy::Error;
    /// Microsteps per `send` before declaring a livelock.
    const MAX_MICROSTEPS: u16 = 64;
    /// Whether handlers call [`Cx::in_state`]; when false the active
    /// configuration snapshot is skipped.
    const TRACKS_ACTIVE: bool = false;

    /// The root initial value, before any entry runs.
    ///
    /// A machine whose root `#[initial(with = ..)]` function takes a [`Cx`]
    /// has no value to give here without one: it implements
    /// [`initial_in`](Self::initial_in) instead, and this function only
    /// exists to satisfy the trait.
    fn initial() -> Self;

    /// The root initial value, built with a context: [`Machine::start`]
    /// calls this and not [`initial`](Self::initial).
    ///
    /// The default is `Self::initial()`. An initializer that emits effects,
    /// raises events or reads the global context overrides it. It runs under
    /// the same rules as every other piece of user code of a start: a panic
    /// poisons the machine, and a full effect sink or raise queue is an error
    /// that poisons it too. Its effects come before those of the entries.
    /// `cx.in_state` answers `false` for every state, because no state is
    /// active yet.
    ///
    /// [`Machine::start`]: crate::Machine::start
    fn initial_in<S: Sink<Self::Effect>>(cx: &mut Cx<'_, Self, S>) -> Self {
        let _ = cx;
        Self::initial()
    }

    /// Id of the active leaf (or of the parallel state).
    fn state_id(&self) -> StateId;

    /// Full active configuration.
    fn path(&self) -> StatePath;

    /// Runs the entries from the root down to the initial leaves.
    fn enter_initial<S: Sink<Self::Effect>>(&mut self, cx: &mut Cx<'_, Self, S>);

    /// One microstep: decision (spec 5.1) and, for a transition, execution
    /// (spec 5.3). `slot` is always live on entry and on return.
    fn microstep<S: Sink<Self::Effect>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
        event: &Self::Event,
    ) -> Outcome;

    /// Evaluates the pending `#[always]` functions (spec 5.9).
    fn run_always<S: Sink<Self::Effect>>(
        slot: &mut Slot<Self>,
        cx: &mut Cx<'_, Self, S>,
    ) -> Outcome {
        let _ = (slot, cx);
        Outcome::Stayed
    }

    /// Whether the root is in a `#[final]` state.
    fn is_done(&self) -> bool {
        false
    }

    /// Moves the output out of the root `#[final]` state, or gives the
    /// state back when it is not in one.
    ///
    /// # Errors
    ///
    /// `Err(self)` when the root is not in a `#[final]` state with output.
    fn into_output(self) -> Result<Self::Output, Self> {
        Err(self)
    }

    /// The event of the timer `key`, built from the current data of its
    /// state (spec 7.1), or `None` when `key` belongs to a state instance
    /// that is no longer active (stale: wrong state or epoch).
    fn timer_event(&self, runtime: &Self::Runtime, key: TimerKey) -> Option<Self::Event> {
        let _ = (runtime, key);
        None
    }

    /// Whether the invoke `key` belongs to the active state instance
    /// (spec 7.2); a result for any other key is stale.
    fn accepts_invoke(&self, runtime: &Self::Runtime, key: InvokeKey) -> bool {
        let _ = (runtime, key);
        false
    }
}
