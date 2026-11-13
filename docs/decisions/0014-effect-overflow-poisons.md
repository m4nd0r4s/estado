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

`DeferOverflow` is not changed: it refuses one event before anything of it ran and the machine stays consistent, though it can follow transitions of the same step.

## Consequences

- A machine that can overflow its sink is a modelling or sizing bug (`fx_inline`, or the sink of the caller), like a full raise queue. Without `alloc`, size `fx_inline` for the largest burst of one step, or pass a sink that cannot overflow.
- The rustdoc and `Display` of `SendError::EffectOverflow`, `docs/reference/errors.md`, and the guide pages on errors and sinks say so.
- Tests: `crates/estado-macros/tests/effect_overflow.rs` (handler, entry and later microstep) and `implicit_start.rs` (start).
