//! Edge enums of the `Tree` handlers and their transition code (spec 5.2,
//! 5.3). One enum per handler, one variant per `goto!` (ADR 0001).

// Variants are named after their targets, so many share a `To` prefix; the
// macro adds the same allow to generated edge enums.
#![allow(clippy::enum_variant_names)]

use estado_core::{Edge, Sink, Slot, StateId};

use super::actions::*;
use super::model::*;

fn active_mut(slot: &mut Slot<Tree>) -> &mut Active {
    match slot.live_mut() {
        Tree::Active(active) => active,
        _ => unreachable!("Active is active"),
    }
}

fn playing_mut(slot: &mut Slot<Tree>) -> &mut Playing {
    match &mut active_mut(slot).sub {
        ActiveSub::Playing(playing) => playing,
        ActiveSub::Paused => unreachable!("Playing is active"),
    }
}

/// Leaves the whole `Active` subtree and writes a new root value built from
/// what exits (LCA = root).
fn leave_active<S: Sink<()>>(
    slot: &mut Slot<Tree>,
    cx: &mut TreeCx<'_, S>,
    construct: impl FnOnce(ActiveExiting, &mut TreeCx<'_, S>) -> Tree,
) {
    exit_active(active_mut(slot), cx);
    let exiting = take_active_recording_history(slot, cx);
    let target = construct(exiting, cx);
    slot.put(target);
    enter_root_state(slot.live_mut(), cx);
}

/// Edges of `#[on(Normal)]`.
pub enum NormalEdges {
    /// Cousin: LCA `Active`.
    ToPaused,
    /// Sibling: LCA `Playing`.
    ToFast,
    /// Local transition to the ancestor `Active`: LCA `Active`.
    ToActiveLocal,
    /// `goto!(Active { count: 100 }, external)`: LCA root.
    ToActiveExternal,
    /// `goto!(Normal)`: LCA `Playing`, exit and enter `Normal`.
    ToItself,
}

impl Edge<Tree> for NormalEdges {
    const TARGETS: &'static [StateId] = &[PAUSED, FAST, ACTIVE, ACTIVE, NORMAL];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Tree>, cx: &mut TreeCx<'_, S>) -> bool {
        match self {
            Self::ToPaused => {
                exit_playing(playing_mut(slot), cx);
                active_mut(slot).sub = ActiveSub::Paused;
                note(cx, "entry Paused");
            }
            Self::ToFast => {
                exit_playing_leaf(playing_mut(slot), cx);
                playing_mut(slot).sub = PlayingSub::Fast;
                enter_playing_leaf(playing_mut(slot), cx);
            }
            Self::ToActiveLocal => {
                exit_playing(playing_mut(slot), cx);
                active_mut(slot).sub = ActiveSub::Playing(initial_playing(PlayingSub::Normal));
                enter_active_sub(&active_mut(slot).sub, cx);
            }
            Self::ToActiveExternal => leave_active(slot, cx, |_, _| {
                Tree::Active(Active {
                    count: 100,
                    sub: ActiveSub::Playing(initial_playing(PlayingSub::Normal)),
                })
            }),
            Self::ToItself => {
                exit_playing_leaf(playing_mut(slot), cx);
                enter_playing_leaf(playing_mut(slot), cx);
                return false;
            }
        }
        true
    }
}

/// Edges of `#[on(Playing)]` (`reenter!()` is handled by the dispatch).
pub enum PlayingEdges {
    ToIdle,
    /// LCA `Active`, from whichever leaf is active.
    ToPaused,
}

impl Edge<Tree> for PlayingEdges {
    const TARGETS: &'static [StateId] = &[IDLE, PAUSED];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Tree>, cx: &mut TreeCx<'_, S>) -> bool {
        match self {
            Self::ToIdle => leave_active(slot, cx, |_, _| Tree::Idle),
            Self::ToPaused => {
                exit_playing(playing_mut(slot), cx);
                active_mut(slot).sub = ActiveSub::Paused;
                note(cx, "entry Paused");
            }
        }
        true
    }
}

/// Edges of `#[on(Active)]`; the constructor closure makes it generic.
pub enum ActiveEdges<StoppedConstructor> {
    /// `goto!(Playing { speed: 1 } / Fast)`: descendant of the handler.
    ToFast,
    /// `goto!(Stopped, |old: Exiting![Active]| Tree::Stopped { .. })`.
    ToStopped(StoppedConstructor),
}

impl<StoppedConstructor: FnOnce(ActiveExiting) -> Tree> Edge<Tree>
    for ActiveEdges<StoppedConstructor>
{
    const TARGETS: &'static [StateId] = &[FAST, STOPPED];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Tree>, cx: &mut TreeCx<'_, S>) -> bool {
        match self {
            Self::ToFast => {
                exit_active_sub(&active_mut(slot).sub, cx);
                active_mut(slot).sub = ActiveSub::Playing(initial_playing(PlayingSub::Fast));
                enter_active_sub(&active_mut(slot).sub, cx);
            }
            Self::ToStopped(constructor) => leave_active(slot, cx, |exiting, cx| {
                note(cx, "construct Stopped");
                constructor(exiting)
            }),
        }
        true
    }
}

/// Edges of `#[on(Idle)]` and `#[on(Stopped)]`.
pub enum OutsideEdges {
    /// `goto!(Active { count: 0 })`.
    ToActive,
    /// `goto!(Active { count: 0 }, history)`: deep history of `Active`.
    ToActiveWithHistory,
}

impl Edge<Tree> for OutsideEdges {
    const TARGETS: &'static [StateId] = &[ACTIVE, ACTIVE];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Tree>, cx: &mut TreeCx<'_, S>) -> bool {
        exit_root_state(slot.live_mut(), cx);
        let sub = match self {
            Self::ToActive => None,
            Self::ToActiveWithHistory => cx.runtime_mut().active_history.take(),
        }
        .unwrap_or_else(|| ActiveSub::Playing(initial_playing(PlayingSub::Normal)));
        slot.put(Tree::Active(Active { count: 0, sub }));
        enter_root_state(slot.live_mut(), cx);
        true
    }
}

/// Edges of `#[on(Paused)]`.
pub enum PausedEdges {
    /// `goto!(Playing, history)`: shallow history of `Playing`, data from
    /// `#[history_init]`.
    ToPlayingWithHistory,
}

impl Edge<Tree> for PausedEdges {
    const TARGETS: &'static [StateId] = &[PLAYING];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Tree>, cx: &mut TreeCx<'_, S>) -> bool {
        match self {
            Self::ToPlayingWithHistory => {
                note(cx, "exit Paused");
                let sub = cx
                    .runtime_mut()
                    .playing_history
                    .map_or(PlayingSub::Normal, PlayingSub::from_discriminant);
                active_mut(slot).sub = ActiveSub::Playing(initial_playing(sub));
                enter_active_sub(&active_mut(slot).sub, cx);
            }
        }
        true
    }
}
