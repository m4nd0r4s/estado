//! Edge enums of the handlers and their transition code (spec 5.2, 5.3).
//!
//! One enum per handler. Every `goto!` of this machine is of the simple form
//! (the target is plain data built in the decision phase), so no variant
//! carries a constructor closure and no enum is generic; scenario I covers
//! constructor edges.

// Variants are named after their targets, so many share a `To` prefix; the
// macro adds the same allow to generated edge enums.
#![allow(clippy::enum_variant_names)]

use estado_core::{Edge, Sink, Slot, StateId};

use super::actions::{
    ConnCx, enter_keepalive, enter_link, enter_root, exit_keepalive, exit_link, exit_root,
};
use super::model::{Command, Conn, Connected, Frame, Keepalive, Link, Session};
use super::tasks::{cancel_keepalive, cancel_link, start_keepalive, start_link};

/// An edge of a region handler. The macro knows statically whether the LCA
/// of each variant is above the parallel state, which is the short-circuit
/// of spec 5.6 item 3.
pub trait RegionEdge: Edge<Conn> {
    /// Whether applying this edge exits the parallel state `Connected`.
    fn leaves_parallel(&self) -> bool;
}

fn connected_mut(slot: &mut Slot<Conn>) -> &mut Connected {
    match slot.live_mut() {
        Conn::Connected(connected) => connected,
        _ => unreachable!("Connected is active"),
    }
}

/// Simple-form transition with the root as LCA: the exits (all regions in
/// reverse order when `Connected` is the source) and their cancellations run,
/// the value built in the decision phase is written, the entries, epochs and
/// scheduling run (spec 5.3 steps 1 to 3, 6 to 9; there is no history and no
/// constructor, so steps 4 and 5 do not exist).
pub fn replace_root<S: Sink<Command>>(slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>, target: Conn) {
    exit_root(slot.live_mut(), cx);
    slot.put(target);
    enter_root(slot.live_mut(), cx);
}

/// Transition inside the `link` region: only the two leaves exit and enter.
pub fn switch_link<S: Sink<Command>>(slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>, next: Link) {
    let connected = connected_mut(slot);
    exit_link(&mut connected.link, cx);
    cancel_link(&connected.link, cx);
    connected.link = next;
    enter_link(&mut connected.link, cx);
    start_link(&connected.link, cx);
}

/// Transition inside the `keepalive` region.
pub fn switch_keepalive<S: Sink<Command>>(
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    next: Keepalive,
) {
    let connected = connected_mut(slot);
    exit_keepalive(connected.keepalive, cx);
    cancel_keepalive(connected.keepalive, cx);
    connected.keepalive = next;
    enter_keepalive(connected.keepalive, cx);
    start_keepalive(connected.keepalive, cx);
}

/// Edges of `#[on(Disconnected)]`.
pub enum DisconnectedEdges {
    /// `goto!(Connecting { attempt: 0 })`.
    ToConnecting,
}

impl Edge<Conn> for DisconnectedEdges {
    const TARGETS: &'static [StateId] = &[Conn::CONNECTING_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        match self {
            Self::ToConnecting => replace_root(slot, cx, Conn::Connecting { attempt: 0 }),
        }
        true
    }
}

/// Edges of `#[on(Connecting)]`.
pub enum ConnectingEdges {
    /// `goto!(Connected { session })`: both regions start in their initials.
    ToConnected {
        /// Session the dial produced.
        session: Session,
    },
    /// `goto!(Backoff { attempt })` on a failed dial.
    ToBackoff {
        /// Failed dials so far.
        attempt: u8,
    },
    /// `goto!(Disconnected)` on `Disconnect`.
    ToDisconnected,
    /// `goto!(Disconnected)` when the last attempt timed out.
    ToDisconnectedAfterGiveUp,
}

impl Edge<Conn> for ConnectingEdges {
    const TARGETS: &'static [StateId] = &[
        Conn::CONNECTED_ID,
        Conn::BACKOFF_ID,
        Conn::DISCONNECTED_ID,
        Conn::DISCONNECTED_ID,
    ];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        let target = match self {
            Self::ToConnected { session } => Conn::Connected(Connected::fresh(session)),
            Self::ToBackoff { attempt } => Conn::Backoff { attempt },
            Self::ToDisconnected | Self::ToDisconnectedAfterGiveUp => Conn::Disconnected,
        };
        replace_root(slot, cx, target);
        true
    }
}

/// Edges of `#[on(Backoff)]`.
pub enum BackoffEdges {
    /// `goto!(Connecting { attempt })`.
    ToConnecting {
        /// Failed dials so far, the one that just started excluded.
        attempt: u8,
    },
    /// `goto!(Disconnected)`.
    ToDisconnected,
}

impl Edge<Conn> for BackoffEdges {
    const TARGETS: &'static [StateId] = &[Conn::CONNECTING_ID, Conn::DISCONNECTED_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        let target = match self {
            Self::ToConnecting { attempt } => Conn::Connecting { attempt },
            Self::ToDisconnected => Conn::Disconnected,
        };
        replace_root(slot, cx, target);
        true
    }
}

/// Edges of `#[on(Connected)]`, the handler of the parallel state itself.
pub enum ConnectedEdges {
    /// `goto!(Disconnected)`.
    ToDisconnected,
}

impl Edge<Conn> for ConnectedEdges {
    const TARGETS: &'static [StateId] = &[Conn::DISCONNECTED_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        match self {
            Self::ToDisconnected => replace_root(slot, cx, Conn::Disconnected),
        }
        true
    }
}

/// Edges of `#[on(Idle)]`, a leaf of `link`.
pub enum IdleEdges {
    /// `goto!(Sending { frame })`.
    ToSending {
        /// The frame to put in flight.
        frame: Frame,
    },
}

impl Edge<Conn> for IdleEdges {
    const TARGETS: &'static [StateId] = &[Conn::SENDING_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        match self {
            Self::ToSending { frame } => switch_link(slot, cx, Link::Sending { frame }),
        }
        true
    }
}

impl RegionEdge for IdleEdges {
    fn leaves_parallel(&self) -> bool {
        false
    }
}

/// Edges of `#[on(Sending)]`.
pub enum SendingEdges {
    /// `goto!(Idle)`.
    ToIdle,
}

impl Edge<Conn> for SendingEdges {
    const TARGETS: &'static [StateId] = &[Conn::IDLE_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        match self {
            Self::ToIdle => switch_link(slot, cx, Link::Idle),
        }
        true
    }
}

impl RegionEdge for SendingEdges {
    fn leaves_parallel(&self) -> bool {
        false
    }
}

/// Edges of `#[on(Pinging)]`, a leaf of `keepalive`.
pub enum PingingEdges {
    /// `goto!(AwaitingPong)`.
    ToAwaitingPong,
}

impl Edge<Conn> for PingingEdges {
    const TARGETS: &'static [StateId] = &[Conn::AWAITING_PONG_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        match self {
            Self::ToAwaitingPong => switch_keepalive(slot, cx, Keepalive::AwaitingPong),
        }
        true
    }
}

impl RegionEdge for PingingEdges {
    fn leaves_parallel(&self) -> bool {
        false
    }
}

/// Edges of `#[on(AwaitingPong)]`.
pub enum AwaitingPongEdges {
    /// `goto!(Pinging)`.
    ToPinging,
    /// `goto!(Dead)`.
    ToDead,
}

impl Edge<Conn> for AwaitingPongEdges {
    const TARGETS: &'static [StateId] = &[Conn::PINGING_ID, Conn::DEAD_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        match self {
            Self::ToPinging => switch_keepalive(slot, cx, Keepalive::Pinging),
            Self::ToDead => switch_keepalive(slot, cx, Keepalive::Dead),
        }
        true
    }
}

impl RegionEdge for AwaitingPongEdges {
    fn leaves_parallel(&self) -> bool {
        false
    }
}

/// Edges of `#[on(Dead)]`.
pub enum DeadEdges {
    /// `goto!(Backoff { attempt: 0 })`: out of `Connected`, the LCA is the
    /// root, so both regions exit.
    ToBackoff,
}

impl Edge<Conn> for DeadEdges {
    const TARGETS: &'static [StateId] = &[Conn::BACKOFF_ID];

    fn apply<S: Sink<Command>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        match self {
            Self::ToBackoff => replace_root(slot, cx, Conn::Backoff { attempt: 0 }),
        }
        true
    }
}

impl RegionEdge for DeadEdges {
    fn leaves_parallel(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::b_connection::graph::GRAPH;

    /// The graph is the exact union of the edge enums (spec 6.3).
    #[test]
    fn graph_edges_match_the_targets_of_the_edge_enums() {
        let declared: [(StateId, &[StateId]); 9] = [
            (Conn::DISCONNECTED_ID, DisconnectedEdges::TARGETS),
            (Conn::CONNECTING_ID, ConnectingEdges::TARGETS),
            (Conn::BACKOFF_ID, BackoffEdges::TARGETS),
            (Conn::CONNECTED_ID, ConnectedEdges::TARGETS),
            (Conn::IDLE_ID, IdleEdges::TARGETS),
            (Conn::SENDING_ID, SendingEdges::TARGETS),
            (Conn::PINGING_ID, PingingEdges::TARGETS),
            (Conn::AWAITING_PONG_ID, AwaitingPongEdges::TARGETS),
            (Conn::DEAD_ID, DeadEdges::TARGETS),
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
