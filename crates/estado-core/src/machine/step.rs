//! One step of a live machine: the microstep of an event, the policy for its
//! outcome and the drain of raised, requeued and `#[always]` work (spec 5.4).

use super::{Source, tally::Tally};
use crate::context::{Cx, Fault};
use crate::error::SendError;
use crate::report::Outcome;
use crate::runtime::{EventQueues, Runtime};
use crate::sink::Sink;
use crate::slot::Slot;
use crate::spec::{DeferOverflowPolicy, Spec, UnhandledPolicy};

/// One event of a live slot, to completion: the done check, the microstep and
/// the drain. The caller owns the poison guard and the error handling.
#[inline(always)]
pub(super) fn step_live<M: Spec, S: Sink<M::Effect>, T: Tally>(
    slot: &mut Slot<M>,
    ctx: &mut M::Ctx,
    runtime: &mut M::Runtime,
    sink: &mut S,
    event: M::Event,
    report: &mut T,
) -> Result<(), SendError<M::Event>> {
    if slot.live().is_some_and(M::is_done) {
        report.ignore_external();
        return Ok(());
    }
    microstep(slot, ctx, runtime, sink, event, Source::External, report)?;
    drain::<M, S, _>(slot, ctx, runtime, sink, report)
}

pub(super) fn check_fault<Event>(fault: Option<Fault>) -> Result<(), SendError<Event>> {
    match fault {
        None => Ok(()),
        Some(Fault::EffectOverflow) => Err(SendError::EffectOverflow),
        Some(Fault::RaiseOverflow) => Err(SendError::RaiseOverflow),
    }
}

/// One microstep of `event`, then the policy for its outcome (spec 5.4).
#[inline(always)]
pub(super) fn microstep<M: Spec, S: Sink<M::Effect>, T: Tally>(
    slot: &mut Slot<M>,
    ctx: &mut M::Ctx,
    runtime: &mut M::Runtime,
    sink: &mut S,
    event: M::Event,
    source: Source,
    report: &mut T,
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
    report.count(outcome, &source);
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
            (UnhandledPolicy::Ignore, Source::External) => report.ignore_external(),
            (_, Source::Internal) => report.unhandled_internal(),
        },
    }
    Ok(())
}

pub(super) fn defer<M: Spec>(
    runtime: &mut M::Runtime,
    event: M::Event,
) -> Result<(), SendError<M::Event>> {
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
pub(super) fn poison_on_defer_overflow<M, Event>(
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
pub(super) fn drain<M: Spec, S: Sink<M::Effect>, T: Tally>(
    slot: &mut Slot<M>,
    ctx: &mut M::Ctx,
    runtime: &mut M::Runtime,
    sink: &mut S,
    report: &mut T,
) -> Result<(), SendError<M::Event>> {
    loop {
        if report.microsteps_of_step() > M::MAX_MICROSTEPS {
            return Err(SendError::Livelock {
                microsteps: report.microsteps_of_step(),
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
            report.count(outcome, &Source::Internal);
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
