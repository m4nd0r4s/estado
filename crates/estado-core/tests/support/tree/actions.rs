//! Entries, exits and history recording of `Tree`, as generated linear code.

use estado_core::{Cx, Sink};

use super::model::{Active, ActiveSub, Playing, PlayingSub, Tree};

pub type TreeCx<'step, S> = Cx<'step, Tree, S>;

pub fn note<S: Sink<()>>(cx: &mut TreeCx<'_, S>, line: impl Into<String>) {
    cx.ctx_mut().push(line.into());
}

fn leaf_name(sub: PlayingSub) -> &'static str {
    match sub {
        PlayingSub::Normal => "Normal",
        PlayingSub::Fast => "Fast",
    }
}

// Exits, deepest first (spec 5.3 step 1), with shallow history (step 3).

pub fn exit_playing_leaf<S: Sink<()>>(playing: &Playing, cx: &mut TreeCx<'_, S>) {
    note(cx, format!("exit {}", leaf_name(playing.sub)));
}

pub fn exit_playing<S: Sink<()>>(playing: &Playing, cx: &mut TreeCx<'_, S>) {
    exit_playing_leaf(playing, cx);
    note(cx, "exit Playing");
    cx.runtime_mut().playing_history = Some(playing.sub.discriminant());
}

pub fn exit_active_sub<S: Sink<()>>(sub: &ActiveSub, cx: &mut TreeCx<'_, S>) {
    match sub {
        ActiveSub::Playing(playing) => exit_playing(playing, cx),
        ActiveSub::Paused => note(cx, "exit Paused"),
    }
}

/// Exits `Active` and its active descendants. The deep history value is
/// recorded by the caller when it takes `Active` by value.
pub fn exit_active<S: Sink<()>>(active: &Active, cx: &mut TreeCx<'_, S>) {
    exit_active_sub(&active.sub, cx);
    note(cx, "exit Active");
}

pub fn exit_root_state<S: Sink<()>>(state: &Tree, cx: &mut TreeCx<'_, S>) {
    match state {
        Tree::Idle => note(cx, "exit Idle"),
        Tree::Stopped { .. } => note(cx, "exit Stopped"),
        Tree::Active(active) => exit_active(active, cx),
    }
}

// Entries, from the child of the LCA down (spec 5.3 step 7).

pub fn enter_playing_leaf<S: Sink<()>>(playing: &Playing, cx: &mut TreeCx<'_, S>) {
    note(cx, format!("entry {}", leaf_name(playing.sub)));
}

pub fn enter_playing<S: Sink<()>>(playing: &Playing, cx: &mut TreeCx<'_, S>) {
    note(cx, "entry Playing");
    enter_playing_leaf(playing, cx);
}

pub fn enter_active_sub<S: Sink<()>>(sub: &ActiveSub, cx: &mut TreeCx<'_, S>) {
    match sub {
        ActiveSub::Playing(playing) => enter_playing(playing, cx),
        ActiveSub::Paused => note(cx, "entry Paused"),
    }
}

pub fn enter_active<S: Sink<()>>(active: &Active, cx: &mut TreeCx<'_, S>) {
    note(cx, "entry Active");
    enter_active_sub(&active.sub, cx);
}

pub fn enter_root_state<S: Sink<()>>(state: &Tree, cx: &mut TreeCx<'_, S>) {
    match state {
        Tree::Idle => note(cx, "entry Idle"),
        Tree::Stopped { .. } => note(cx, "entry Stopped"),
        Tree::Active(active) => enter_active(active, cx),
    }
}

/// Takes `Active` out of the root by value (step 4) and records its deep
/// history (step 3): the substate value moves into the runtime, the rest is
/// returned to the constructor (errata 5).
pub fn take_active_recording_history<S: Sink<()>>(
    slot: &mut estado_core::Slot<Tree>,
    cx: &mut TreeCx<'_, S>,
) -> ActiveExiting {
    let Tree::Active(Active { count, sub }) = slot.take() else {
        unreachable!("Active is active")
    };
    cx.runtime_mut().active_history = Some(sub);
    ActiveExiting { count }
}

/// `Exiting![Active]` for a compound with `#[history(deep, keep)]`: its
/// substates went to the history, the shared fields move to the constructor.
pub struct ActiveExiting {
    pub count: u32,
}
