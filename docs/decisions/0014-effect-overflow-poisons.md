# 0014. An effect overflow poisons the machine

Status: accepted
Date: 2026-11-13
Spec: 4.6, 5.3, 5.11; ADR 0013 amendment 20

## Context

When a sink refuses an effect (`Overflow`), the machine reports `SendError::EffectOverflow`. Until now only a `start` poisoned the machine on it (ADR 0013, amendment 20). During a normal `send` the machine kept the applied transition, lost the refused effect and cut the rest of the step short (raised events, `#[always]`, completions and redelivery never ran), and stayed usable.

That leaves a half-run step behind. The microstep that overflowed had already run: its exits and entries happened and its data changed. Part of its effects reached the sink, the rest did not. Effects are also the contract between the machine and its driver: `StartTimer`, `CancelTimer`, `Invoke` and `Cancel` say what the driver must run. A lost one leaves a timer or a task running for a state that is gone, or never started for the state that is active, and nothing tells the caller which one. Raised and deferred events that were still queued would also run in a later `send`, long after the step they belong to. Spec 5.3 does not define a partial step, and `RaiseOverflow` and `Livelock`, the other faults that stop a step half-way, already poison.

The alternatives were: keep the status quo and document it; roll the microstep back (not possible, the state was moved and the effects already pushed to the sink cannot be recalled); poison.

## Decision

Every `EffectOverflow` poisons the machine: in a `start`, in the first microstep of a `send`, and in any later microstep of the same step (raised event, `#[always]`, completion, redelivery). The error is returned once; every later call returns `Err(SendError::Poisoned)`.

The `Display` text reads "the effect buffer is full; the machine is poisoned". Section 5 does not say otherwise: it only lists panics and the raise overflow as poisoning, and does not promise that a machine survives an overflow.

`DeferOverflow` was left unchanged here; the amendment below settles it.

## Consequences

- A machine that can overflow its sink is a modelling or sizing bug (`fx_inline`, or the sink of the caller), like a full raise queue. Without `alloc`, size `fx_inline` for the largest burst of one step, or pass a sink that cannot overflow.
- The rustdoc and `Display` of `SendError::EffectOverflow`, `docs/reference/errors.md`, and the guide pages on errors and sinks say so.
- Tests: `crates/estado-macros/tests/effect_overflow.rs` (handler, entry and later microstep) and `implicit_start.rs` (start).

## Amendment: a `DeferOverflow` after the first microstep poisons

Date: 2026-11-13. Bead `estado-99i.4.17`.

### Context

The decision above left `DeferOverflow` alone on the grounds that it refuses one event before anything of it ran. That holds for the external event, but a raised or redelivered event is deferred in a later microstep of the same step. If it overflows there, the microsteps before it already ran: exits and entries happened, data changed, effects went out. The step stops half-way, the rest of the raised events, `#[always]`, completions and redelivery never run, and the error carries only the one event that did not fit. That is the same half-run step that made `EffectOverflow`, `RaiseOverflow` and `Livelock` poison, and the reasons of the decision above apply unchanged: a later `send` would run the leftovers of a step that ended long ago, and the caller cannot tell how much of the step ran.

The alternatives were to keep it documented as is, or to poison.

### Decision

A `DeferOverflow` under `defer_overflow = error` poisons the machine when it happens in any microstep after the first one of a `send` (a raised event, an event requeued after a configuration change). The error is returned once, with the event that did not fit; every later call returns `Err(SendError::Poisoned)`.

A `DeferOverflow` in the first microstep, the one of the external event, does not poison. Nothing of the step ran before it: the event is given back, the configuration and the queues are unchanged, and the machine stays usable. This is what spec 5.5 describes ("returns `Err(DeferOverflow(ev))` without changing state"), so the spec text stays right for the external event and only the mid-step case is a deviation (errata 17).

With `defer_overflow = drop_oldest` nothing overflows, so nothing changes.

### Consequences

- A machine that can overflow its defer queue mid-step is a sizing or modelling bug (`defer_cap`), like a full raise queue. Size `defer_cap` for the events that can be deferred in one step, or use `drop_oldest`.
- The rustdoc of `SendError::DeferOverflow`, `docs/reference/errors.md`, and the guide pages on errors and on defer and raise say so.
- Tests: `crates/estado-macros/tests/defer_overflow.rs` (external event, raised event after a handler, raised event after a transition) and `crates/estado-core/tests/send_algorithm.rs` (external event).
