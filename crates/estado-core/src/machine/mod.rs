//! `Machine<M>` and the run-to-completion `send` algorithm (spec 4.5, 5.4).

mod tasks;

use crate::context::{Cx, Fault};
use crate::effect::Effect;
use crate::error::SendError;
use crate::ids::{StateId, StatePath};
use crate::report::{IgnoreReason, Outcome, Report, Step};
use crate::runtime::{EventQueues, Runtime};
use crate::sink::{FnSink, Sink};
use crate::slot::Slot;
use crate::spec::{DeferOverflowPolicy, Spec, UnhandledPolicy};

/// A running instance of the machine `M`.
///
/// Holds exactly the root state, the context and the runtime; the runtime
/// is zero-sized when no optional feature is used (spec 3.5, errata 1).
///
/// A flat on/off machine written by hand against [`Spec`], driven through
/// `send`. The first `send` starts the machine implicitly.
///
/// ```
/// use estado_core::{
///     Cx, Discard, Graph, Machine, NoRuntime, Outcome, Sink, Slot, Spec, StateId, StatePath,
/// };
///
/// #[derive(Debug, Clone, Copy, PartialEq)]
/// enum Switch {
///     Off,
///     On,
/// }
///
/// impl Spec for Switch {
///     type Event = ();
///     type Effect = ();
///     type Ctx = u32; // counts the flips
///     type Output = ();
///     type Runtime = NoRuntime<()>;
///     type Effects = Discard;
///
///     const NAME: &'static str = "Switch";
///     const GRAPH: &'static Graph = &Graph { machine: "Switch", states: &[], edges: &[] };
///
///     fn initial() -> Self {
///         Self::Off
///     }
///     fn state_id(&self) -> StateId {
///         StateId::new(*self as u16)
///     }
///     fn path(&self) -> StatePath {
///         StatePath::new(Self::GRAPH)
///     }
///     fn enter_initial<S: Sink<()>>(&mut self, _cx: &mut Cx<'_, Self, S>) {}
///
///     fn microstep<S: Sink<()>>(
///         slot: &mut Slot<Self>,
///         cx: &mut Cx<'_, Self, S>,
///         _event: &(),
///     ) -> Outcome {
///         *cx.ctx_mut() += 1;
///         let next = match slot.take() {
///             Self::Off => Self::On,
///             Self::On => Self::Off,
///         };
///         slot.put(next);
///         Outcome::Transitioned { changed: true }
///     }
/// }
///
/// let mut machine = Machine::<Switch>::new(0);
/// assert!(machine.try_state().is_none()); // not started yet
///
/// let step = machine.send(()).unwrap();
/// assert!(step.report.handled);
/// assert_eq!(machine.state(), &Switch::On);
/// assert_eq!(*machine.ctx(), 1);
/// ```
pub struct Machine<M: Spec> {
    slot: Slot<M>,
    ctx: M::Ctx,
    runtime: M::Runtime,
}

/// Leaves the slot poisoned unless disarmed with `mem::forget` (spec 5.11).
struct PoisonGuard<'machine, M> {
    slot: &'machine mut Slot<M>,
}

impl<M> Drop for PoisonGuard<'_, M> {
    fn drop(&mut self) {
        *self.slot = Slot::Poisoned;
    }
}

impl<M> PoisonGuard<'_, M> {
    fn disarm(self) {
        core::mem::forget(self);
    }
}

/// Which event a microstep processes.
enum Source {
    External,
    Internal,
}

impl<M: Spec> Machine<M> {
    /// A machine that has not run any entry yet.
    pub const fn new(ctx: M::Ctx) -> Self {
        Self {
            slot: Slot::Unstarted,
            ctx,
            runtime: M::Runtime::NEW,
        }
    }

    /// Runs the initial entries. Does nothing when already started.
    ///
    /// # Errors
    ///
    /// [`SendError::Poisoned`], or a fault raised by an entry.
    pub fn start(&mut self) -> Result<Step<M::Effects>, SendError<M::Event>> {
        let mut effects = M::Effects::default();
        let report = self.start_into(&mut effects)?;
        Ok(Step { effects, report })
    }

    /// [`start`](Self::start) writing effects into `sink`.
    ///
    /// # Errors
    ///
    /// See [`start`](Self::start).
    pub fn start_into<S: Sink<M::Effect>>(
        &mut self,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        match self.slot {
            Slot::Poisoned => return Err(SendError::Poisoned),
            Slot::Live(_) | Slot::Finished => return Ok(Report::default()),
            Slot::Unstarted => {}
        }
        let guard = PoisonGuard {
            slot: &mut self.slot,
        };
        let mut report = Report::default();
        // A start that fails half-way (entries ran, `#[always]` or raised
        // events still pending) leaves no consistent configuration to resume
        // from, so every start error poisons (ADR 0013, amendment by the lead).
        // Dropping the guard writes `Poisoned`.
        if let Err(error) = start_in(guard.slot, &mut self.ctx, &mut self.runtime, sink) {
            drop(guard);
            return Err(error);
        }
        if let Err(error) = drain::<M, S>(
            guard.slot,
            &mut self.ctx,
            &mut self.runtime,
            sink,
            &mut report,
        ) {
            drop(guard);
            return Err(error);
        }
        guard.disarm();
        Ok(report)
    }

    /// Processes one event to completion; effects are returned in the step.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn send(&mut self, event: M::Event) -> Result<Step<M::Effects>, SendError<M::Event>> {
        let mut effects = M::Effects::default();
        let report = self.send_into(event, &mut effects)?;
        Ok(Step { effects, report })
    }

    /// Processes one event to completion, writing effects into `sink`.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    #[inline]
    pub fn send_into<S: Sink<M::Effect>>(
        &mut self,
        event: M::Event,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        // One test on the hot path; the other slot states go out of line.
        if !matches!(self.slot, Slot::Live(_)) {
            return self.send_when_not_live(event, sink);
        }
        self.send_live(event, sink)
    }

    /// `send_into` on a live slot.
    #[inline(always)]
    fn send_live<S: Sink<M::Effect>>(
        &mut self,
        event: M::Event,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        self.send_live_from(event, sink, Report::default())
    }

    /// `send_live` continuing the counters of `report`: after an implicit
    /// start, the start and the event share one livelock budget.
    #[inline(always)]
    fn send_live_from<S: Sink<M::Effect>>(
        &mut self,
        event: M::Event,
        sink: &mut S,
        mut report: Report,
    ) -> Result<Report, SendError<M::Event>> {
        let guard = PoisonGuard {
            slot: &mut self.slot,
        };
        if guard.slot.live().is_some_and(M::is_done) {
            report.ignored = Some(IgnoreReason::Unhandled);
            guard.disarm();
            return Ok(report);
        }
        let first = microstep(
            guard.slot,
            &mut self.ctx,
            &mut self.runtime,
            sink,
            event,
            Source::External,
            &mut report,
        );
        if let Err(error) = first {
            return Err(settle_error(guard, error));
        }
        if let Err(error) = drain::<M, S>(
            guard.slot,
            &mut self.ctx,
            &mut self.runtime,
            sink,
            &mut report,
        ) {
            return Err(settle_error(guard, error));
        }
        guard.disarm();
        Ok(report)
    }

    /// The slow path of `send_into`: refuses a poisoned machine, ignores
    /// events after the output was taken (spec 5.10), and runs the initial
    /// entries of an unstarted one before the event (spec 4.5).
    #[cold]
    #[inline(never)]
    fn send_when_not_live<S: Sink<M::Effect>>(
        &mut self,
        event: M::Event,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        match self.slot {
            Slot::Live(_) => {}
            Slot::Poisoned => return Err(SendError::Poisoned),
            Slot::Finished => {
                return Ok(Report {
                    ignored: Some(IgnoreReason::Unhandled),
                    ..Report::default()
                });
            }
            Slot::Unstarted => {
                // The full start, `#[always]`, completions and raised events
                // included, runs before the event (spec 4.5, ADR 0013 item 6),
                // within one livelock budget.
                let started = self.start_into(sink)?;
                return self.send_live_from(event, sink, started);
            }
        }
        self.send_live(event, sink)
    }

    /// [`send_into`](Self::send_into) with a closure as the sink.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn send_with(
        &mut self,
        event: M::Event,
        on_effect: impl FnMut(Effect<M::Effect>),
    ) -> Result<Report, SendError<M::Event>> {
        self.send_into(event, &mut FnSink(on_effect))
    }

    /// Sends every event in order; stops at the first error. The report
    /// accumulates counters; `handled` and `ignored` describe the last event.
    ///
    /// # Errors
    ///
    /// See [`SendError`].
    pub fn send_batch<S: Sink<M::Effect>>(
        &mut self,
        events: impl IntoIterator<Item = M::Event>,
        sink: &mut S,
    ) -> Result<Report, SendError<M::Event>> {
        let mut total = Report::default();
        // Wide counters, clamped once: the same value as saturating at every
        // event (all addends are non-negative), without the compare per event.
        let mut transitions = 0_u64;
        let mut microsteps = 0_u64;
        let mut unhandled_internal = 0_u64;
        for event in events {
            let report = self.send_into(event, sink)?;
            total.handled = report.handled;
            total.ignored = report.ignored;
            transitions += u64::from(report.transitions);
            microsteps += u64::from(report.microsteps);
            unhandled_internal += u64::from(report.unhandled_internal);
        }
        let clamp = |count: u64| u16::try_from(count).unwrap_or(u16::MAX);
        total.transitions = clamp(transitions);
        total.microsteps = clamp(microsteps);
        total.unhandled_internal = clamp(unhandled_internal);
        Ok(total)
    }

    /// The root state.
    ///
    /// # Panics
    ///
    /// If the machine is poisoned or not started.
    pub fn state(&self) -> &M {
        match &self.slot {
            Slot::Live(state) => state,
            Slot::Unstarted => panic!(
                "estado: machine `{}` has not started; call start() or send()",
                M::NAME
            ),
            Slot::Poisoned => panic!(
                "estado: machine `{}` is poisoned by an earlier panic or fatal error",
                M::NAME
            ),
            Slot::Finished => panic!(
                "estado: machine `{}` finished and its output was taken",
                M::NAME
            ),
        }
    }

    /// The root state, unless poisoned or not started.
    pub fn try_state(&self) -> Option<&M> {
        self.slot.live()
    }

    /// The global context.
    pub fn ctx(&self) -> &M::Ctx {
        &self.ctx
    }

    /// The global context, mutably.
    pub fn ctx_mut(&mut self) -> &mut M::Ctx {
        &mut self.ctx
    }

    /// Whether `state` is active. `false` when not live.
    pub fn is_in(&self, state: StateId) -> bool {
        self.slot
            .live()
            .is_some_and(|root| root.path().contains(state))
    }

    /// The active configuration.
    ///
    /// # Panics
    ///
    /// If the machine is poisoned or not started.
    pub fn path(&self) -> StatePath {
        self.state().path()
    }

    /// Whether the root reached a `#[final]` state.
    pub fn is_done(&self) -> bool {
        match &self.slot {
            Slot::Live(state) => state.is_done(),
            Slot::Finished => true,
            Slot::Unstarted | Slot::Poisoned => false,
        }
    }

    /// Moves the output out of the root `#[final]` state. The machine is
    /// then finished: still done, its state no longer readable, and later
    /// calls return `None`.
    pub fn take_output(&mut self) -> Option<M::Output> {
        if !self.slot.live().is_some_and(M::is_done) {
            return None;
        }
        match self.slot.take().into_output() {
            Ok(output) => {
                self.slot = Slot::Finished;
                Some(output)
            }
            Err(state) => {
                self.slot.put(state);
                None
            }
        }
    }

    /// Whether the machine is poisoned.
    pub fn is_poisoned(&self) -> bool {
        self.slot.is_poisoned()
    }

    /// The runtime, for generated code and tests.
    #[doc(hidden)]
    pub fn runtime(&self) -> &M::Runtime {
        &self.runtime
    }
}

/// Ends a failed step: poisons for a fatal error, otherwise leaves the state.
///
/// A fault found while a step runs (a full raise queue, a full effect sink, a
/// livelock) stops the step half-way, after transitions ran and effects went
/// out, so none of them leaves a configuration to resume from (ADR 0014). A
/// `DeferOverflow` of the external event refused it before anything ran and
/// does not poison; one of a raised or requeued event does, and `drain`
/// poisons the slot itself before it gets here.
fn settle_error<M, Event>(guard: PoisonGuard<'_, M>, error: SendError<Event>) -> SendError<Event> {
    let poisons = matches!(
        error,
        SendError::RaiseOverflow | SendError::EffectOverflow | SendError::Livelock { .. }
    );
    if poisons {
        drop(guard);
    } else {
        guard.disarm();
    }
    error
}

fn start_in<M: Spec, S: Sink<M::Effect>>(
    slot: &mut Slot<M>,
    ctx: &mut M::Ctx,
    runtime: &mut M::Runtime,
    sink: &mut S,
) -> Result<(), SendError<M::Event>> {
    // The root initializer is user code and may emit effects: it runs inside
    // the start, under the guard of its caller, before the entries. No state is
    // active yet, so `in_state` sees an empty configuration.
    let mut nothing_active = if M::TRACKS_ACTIVE {
        Some(StatePath::new(M::GRAPH))
    } else {
        None
    };
    let initial = {
        let mut cx = Cx::<M, S>::new(ctx, runtime, sink, nothing_active.as_mut());
        let initial = M::initial_in(&mut cx);
        check_fault(cx.fault())?;
        initial
    };
    slot.put(initial);
    // Initial entries see the configuration being entered (ADR 0013 item 7).
    let mut snapshot = if M::TRACKS_ACTIVE {
        Some(slot.live_mut().path())
    } else {
        None
    };
    let mut cx = Cx::<M, S>::new(ctx, runtime, sink, snapshot.as_mut());
    slot.live_mut().enter_initial(&mut cx);
    check_fault(cx.fault())
}

fn check_fault<Event>(fault: Option<Fault>) -> Result<(), SendError<Event>> {
    match fault {
        None => Ok(()),
        Some(Fault::EffectOverflow) => Err(SendError::EffectOverflow),
        Some(Fault::RaiseOverflow) => Err(SendError::RaiseOverflow),
    }
}

/// One microstep of `event`, then the policy for its outcome (spec 5.4).
#[inline(always)]
fn microstep<M: Spec, S: Sink<M::Effect>>(
    slot: &mut Slot<M>,
    ctx: &mut M::Ctx,
    runtime: &mut M::Runtime,
    sink: &mut S,
    event: M::Event,
    source: Source,
    report: &mut Report,
) -> Result<(), SendError<M::Event>> {
    let mut snapshot = if M::TRACKS_ACTIVE {
        Some(slot.live_mut().path())
    } else {
        None
    };
    let outcome = {
        let mut cx = Cx::<M, S>::new(ctx, runtime, sink, snapshot.as_mut());
        let outcome = M::microstep(slot, &mut cx, &event);
        check_fault(cx.fault())?;
        outcome
    };
    count(report, outcome, &source);
    match outcome {
        Outcome::Stayed => {}
        Outcome::Transitioned { changed } => {
            if changed {
                runtime.queues().requeue_deferred();
            }
        }
        Outcome::Deferred => defer::<M>(runtime, event)?,
        Outcome::Unhandled(policy) => match (policy, source) {
            (UnhandledPolicy::Panic, _) => panic!(
                "estado: machine `{}` has no handler for the event in {}",
                M::NAME,
                slot.live_mut().path()
            ),
            (UnhandledPolicy::Error, Source::External) => return Err(SendError::Unhandled(event)),
            (UnhandledPolicy::Ignore, Source::External) => {
                report.ignored = Some(IgnoreReason::Unhandled)
            }
            (_, Source::Internal) => report.unhandled_internal += 1,
        },
    }
    Ok(())
}

#[inline(always)]
fn count(report: &mut Report, outcome: Outcome, source: &Source) {
    report.microsteps += 1;
    if let Outcome::Transitioned { .. } = outcome {
        report.transitions += 1;
    }
    if matches!(source, Source::External) {
        report.handled = !matches!(outcome, Outcome::Unhandled(_));
    }
}

fn defer<M: Spec>(runtime: &mut M::Runtime, event: M::Event) -> Result<(), SendError<M::Event>> {
    let queues = runtime.queues();
    match queues.defer(event) {
        Ok(()) => Ok(()),
        Err(event) => match M::DEFER_OVERFLOW {
            DeferOverflowPolicy::Error => Err(SendError::DeferOverflow(event)),
            DeferOverflowPolicy::DropOldest => {
                queues.drop_oldest_deferred();
                queues.defer(event).map_err(SendError::DeferOverflow)
            }
        },
    }
}

/// A `DeferOverflow` of a raised or requeued event comes after the microsteps
/// that raised or requeued it, which already ran and sent their effects: the
/// step is cut half-way, so it poisons like the other faults (ADR 0014,
/// amendment). The same error of the external event does not pass through
/// here: it refused the event before anything of the step ran.
#[cold]
fn poison_on_defer_overflow<M, Event>(
    slot: &mut Slot<M>,
    error: SendError<Event>,
) -> SendError<Event> {
    if matches!(error, SendError::DeferOverflow(_)) {
        *slot = Slot::Poisoned;
    }
    error
}

/// Runs pending always > internal > requeued events until none is left.
#[inline(always)]
fn drain<M: Spec, S: Sink<M::Effect>>(
    slot: &mut Slot<M>,
    ctx: &mut M::Ctx,
    runtime: &mut M::Runtime,
    sink: &mut S,
    report: &mut Report,
) -> Result<(), SendError<M::Event>> {
    loop {
        if report.microsteps > M::MAX_MICROSTEPS {
            return Err(SendError::Livelock {
                microsteps: report.microsteps,
                last: slot.live_mut().path(),
            });
        }
        if runtime.take_always_pending() {
            let outcome = {
                // Same snapshot as an event microstep (spec 4.3, errata 11).
                let mut snapshot = if M::TRACKS_ACTIVE {
                    Some(slot.live_mut().path())
                } else {
                    None
                };
                let mut cx = Cx::<M, S>::new(ctx, runtime, sink, snapshot.as_mut());
                let outcome = M::run_always(slot, &mut cx);
                check_fault(cx.fault())?;
                outcome
            };
            count(report, outcome, &Source::Internal);
            if let Outcome::Transitioned { changed: true } = outcome {
                runtime.queues().requeue_deferred();
            }
            continue;
        }
        let next = {
            let queues = runtime.queues();
            queues.pop_raised().or_else(|| queues.pop_requeued())
        };
        match next {
            Some(event) => {
                if let Err(error) =
                    microstep(slot, ctx, runtime, sink, event, Source::Internal, report)
                {
                    return Err(poison_on_defer_overflow(slot, error));
                }
            }
            None => return Ok(()),
        }
    }
}
