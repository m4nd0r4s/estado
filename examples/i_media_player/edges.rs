//! Edge enums of the handlers and their transition code (spec 5.2, 5.3).
//!
//! One enum per handler, one variant per `goto!` (ADR 0001). Variants of the
//! simple form carry plain data; variants of the constructor form carry the
//! closure, so the enum is generic over its type.

// Variants are named after their targets, so many share a `To` prefix; the
// macro adds the same allow to generated edge enums.
#![allow(clippy::enum_variant_names)]

use estado_core::{Edge, Sink, Slot, Spec, StateId};

use super::actions::{
    PlayerCx, enter_active_sub, enter_root_state, exit_active_sub, exit_failed, exit_root_state,
    note,
};
use super::model::{Active, ActiveSub, AudioBuffer, Command, Player, TrackId};
use super::views::{ActiveExiting, FailedExiting, FailedMut};

fn active_mut(slot: &mut Slot<Player>) -> &mut Active {
    match slot.live_mut() {
        Player::Active(active) => active,
        _ => unreachable!("Active is active"),
    }
}

/// Exits `Active` and everything below it, takes it out of the root by
/// value, lets `construct` build the new root value from it and enters the
/// result (LCA = root, spec 5.3 steps 1, 4, 5, 6 and 7).
fn leave_active<S: Sink<Command>>(
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
    construct: impl FnOnce(ActiveExiting, &mut PlayerCx<'_, S>) -> Player,
) {
    exit_root_state(slot.live_mut(), cx);
    let Player::Active(exiting) = slot.take() else {
        unreachable!("Active is active")
    };
    let target = construct(exiting, cx);
    slot.put(target);
    enter_root_state(slot.live_mut(), cx);
}

/// The same for `Failed`: the generated `FailedExiting` carries its fields.
fn leave_failed<S: Sink<Command>>(
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
    construct: impl FnOnce(FailedExiting, &mut PlayerCx<'_, S>) -> Player,
) {
    let Player::Failed { reason, buffer } = slot.live_mut() else {
        unreachable!("Failed is active")
    };
    exit_failed(FailedMut { reason, buffer }, cx);
    let Player::Failed { reason, buffer } = slot.take() else {
        unreachable!("Failed is active")
    };
    let target = construct(FailedExiting { reason, buffer }, cx);
    slot.put(target);
    enter_root_state(slot.live_mut(), cx);
}

/// Replaces the substate of `Active` (LCA `Active`).
fn switch_substate<S: Sink<Command>>(
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
    next: ActiveSub,
) {
    let active = active_mut(slot);
    exit_active_sub(&mut active.sub, cx);
    active.sub = next;
    enter_active_sub(&mut active.sub, cx);
}

/// Replaces the whole root value without a constructor (simple form of
/// `goto!`, LCA = root): the exits run, the value built in the decision phase
/// is written, the entries run.
fn replace_root<S: Sink<Command>>(
    slot: &mut Slot<Player>,
    cx: &mut PlayerCx<'_, S>,
    target: Player,
) {
    exit_root_state(slot.live_mut(), cx);
    slot.put(target);
    enter_root_state(slot.live_mut(), cx);
}

/// Edges of `#[on(Idle)]`.
pub enum IdleEdges {
    /// `goto!(Active { track, buf })`: the substate is the initial.
    ToActive { track: TrackId, buf: AudioBuffer },
}

impl Edge<Player> for IdleEdges {
    const TARGETS: &'static [StateId] = &[Player::ACTIVE];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Player>, cx: &mut PlayerCx<'_, S>) -> bool {
        match self {
            Self::ToActive { track, buf } => {
                let sub = ActiveSub::initial();
                replace_root(slot, cx, Player::Active(Active { track, buf, sub }));
            }
        }
        true
    }
}

/// Edges of `#[on(Playing)]`.
pub enum PlayingEdges {
    /// `goto!(Paused { pos })`: sibling, LCA `Active`.
    ToPaused { pos: std::time::Duration },
}

impl Edge<Player> for PlayingEdges {
    const TARGETS: &'static [StateId] = &[Player::PAUSED];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Player>, cx: &mut PlayerCx<'_, S>) -> bool {
        match self {
            Self::ToPaused { pos } => switch_substate(slot, cx, ActiveSub::Paused { pos }),
        }
        true
    }
}

/// Edges of `#[on(Paused)]`.
pub enum PausedEdges {
    /// `goto!(Playing { pos })`: sibling, LCA `Active`.
    ToPlaying { pos: std::time::Duration },
}

impl Edge<Player> for PausedEdges {
    const TARGETS: &'static [StateId] = &[Player::PLAYING];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Player>, cx: &mut PlayerCx<'_, S>) -> bool {
        match self {
            Self::ToPlaying { pos } => switch_substate(slot, cx, ActiveSub::Playing { pos }),
        }
        true
    }
}

/// Edges of `#[on(Active)]`; the constructor closure makes it generic.
pub enum ActiveEdges<FailedConstructor> {
    /// `goto!(Idle)`: LCA root.
    ToIdle,
    /// `goto!(Active { track, buf })` from the handler of `Active` itself:
    /// exits and enters `Active`, which comes back with a new track.
    ToActive { track: TrackId, buf: AudioBuffer },
    /// `goto!(Failed, |old: Exiting![Active]| Player::Failed { .. })`.
    ToFailed(FailedConstructor),
}

impl<FailedConstructor: FnOnce(ActiveExiting) -> Player> Edge<Player>
    for ActiveEdges<FailedConstructor>
{
    const TARGETS: &'static [StateId] = &[Player::IDLE, Player::ACTIVE, Player::FAILED];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Player>, cx: &mut PlayerCx<'_, S>) -> bool {
        match self {
            Self::ToIdle => replace_root(slot, cx, Player::Idle),
            Self::ToActive { track, buf } => {
                let before = slot.live_mut().state_id();
                let sub = ActiveSub::initial();
                replace_root(slot, cx, Player::Active(Active { track, buf, sub }));
                return slot.live_mut().state_id() != before;
            }
            Self::ToFailed(constructor) => leave_active(slot, cx, |exiting, cx| {
                note(cx, "construct Failed");
                constructor(exiting)
            }),
        }
        true
    }
}

/// Edges of `#[on(Failed)]`.
pub enum FailedEdges<ActiveConstructor> {
    /// `goto!(Active, |old: Exiting![Failed]| Player::Active(..))`.
    ToActive(ActiveConstructor),
    /// `goto!(Idle)`: the buffer is dropped with `Failed`.
    ToIdle,
}

impl<ActiveConstructor: FnOnce(FailedExiting) -> Player> Edge<Player>
    for FailedEdges<ActiveConstructor>
{
    const TARGETS: &'static [StateId] = &[Player::ACTIVE, Player::IDLE];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Player>, cx: &mut PlayerCx<'_, S>) -> bool {
        match self {
            Self::ToActive(constructor) => leave_failed(slot, cx, |exiting, cx| {
                note(cx, "construct Active");
                constructor(exiting)
            }),
            Self::ToIdle => replace_root(slot, cx, Player::Idle),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i_media_player::graph::GRAPH;

    /// The graph is the exact union of the edge enums (spec 6.3).
    #[test]
    fn graph_edges_match_the_targets_of_the_edge_enums() {
        type NoConstructor = fn(ActiveExiting) -> Player;
        type NoFailedConstructor = fn(FailedExiting) -> Player;
        let declared: [(StateId, &[StateId]); 5] = [
            (Player::IDLE, IdleEdges::TARGETS),
            (Player::PLAYING, PlayingEdges::TARGETS),
            (Player::PAUSED, PausedEdges::TARGETS),
            (Player::ACTIVE, <ActiveEdges<NoConstructor>>::TARGETS),
            (Player::FAILED, <FailedEdges<NoFailedConstructor>>::TARGETS),
        ];
        for (from, targets) in declared {
            let in_graph: Vec<_> = GRAPH
                .edges
                .iter()
                .filter(|edge| edge.from == from)
                .map(|edge| edge.to)
                .collect();
            let in_enum: Vec<_> = targets
                .iter()
                .map(|target| estado_core::EdgeTarget::State(*target))
                .collect();
            assert_eq!(in_graph, in_enum, "edges from {from:?}");
        }
    }
}
