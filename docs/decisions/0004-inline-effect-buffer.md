# 0004. Own inline effect buffer, no `smallvec`

Status: accepted
Date: 2026-11-12
Spec: 4.4, 4.5, 13.3

## Context

`Step` holds the effects of a `send` in a buffer with `fx_inline` slots inline (default 4) that spills to the heap with `alloc` and returns `EffectOverflow` without it. The core may have at most two dependencies, all optional (spec 2.2). Options: `smallvec` as a dependency, or an own implementation.

## Decision

Own implementation, `InlineEffects<Command, CAPACITY>`: `[Option<Effect<Command>>; CAPACITY]` plus a length, and a `Vec` spill field that exists only with the `alloc` feature. No `unsafe`: `Option<Effect<_>>` uses the niche of the `Effect` discriminant, so the `Option` costs no space. The same pattern gives `InlineQueue` for the raise, defer and requeue queues.

The per-machine capacity is the `Spec::Effects` associated type (`InlineEffects<Cmd, 4>`), since a `const` associated item cannot size an array on stable.

## Consequences

- `estado-core` keeps zero mandatory and zero optional dependencies so far; both slots stay free (`heapless` sink support is the likely candidate).
- `Sink` is implemented for `InlineEffects`, `Vec`, `&mut impl Sink`, `FnSink(closure)` and `Discard`. A blanket impl for every `FnMut` would conflict with the `Vec` impl under coherence rules, so closures go through the `FnSink` wrapper; `send_with` wraps for the caller.
- `Sink` is generic over the command type (`Sink<Command>`) rather than over the `Spec`, so child machines can reuse a parent's sink through `Effect::map`.
