# 0015. `cx.in_state` on an untracked machine panics

Status: accepted
Date: 2026-11-13
Spec: 4.3, 6.5; ADR 0013 item 8

## Context

`cx.in_state(id)` reads a snapshot of the active configuration. The snapshot costs a `StatePath` per microstep, so `#[machine]` keeps it only when it sees `in_state` in the machine module (ADR 0013 item 8). A call from a helper outside the module is invisible to that scan. Until now such a call returned `false` for every state, which is a wrong answer that looks like a right one: a guard such as `if cx.in_state(Self::OPEN_ID)` is silently never true.

Options:

1. Track in every machine. A per-step cost for machines that never ask, against the zero-cost goal and BM-01.
2. Track whenever some handler takes a `cx`. Almost every machine does, so this is option 1 with extra steps.
3. Emit a diagnostic. The macro cannot see the call, so it cannot know to warn.
4. Make the untracked call fail loudly.

## Decision

Option 4. `Cx::in_state` panics when the machine does not track the active configuration. The message names the remedy: `tracks_active` in `#[machine(..)]`, or `Spec::TRACKS_ACTIVE` in a hand-written `Spec`. The panic poisons the machine like any panic in a handler, and the first test that reaches the call finds it.

The detection of ADR 0013 item 8 stays as it is: most machines never need the option.

## Consequences

- `in_state` can no longer return a wrong `false`. A machine that tracks gives the real answer; one that does not stops at the first call.
- The failure is at run time, not at compile time, because the macro cannot see helpers outside the module. A branch that never runs hides the mistake until it does; the message and the docs say how to fix it.
- Docs: `docs/reference/context.md`, `docs/reference/attributes.md`, `docs/guide/22-warnings-and-options.md`, the rustdoc of `Cx::in_state` and of `#[machine]`.
- Test: `crates/estado-macros/tests/in_state_untracked.rs`.
