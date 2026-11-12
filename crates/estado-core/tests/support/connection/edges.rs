//! Entries, exits and the transition code of spec 5.3, as the macro would
//! generate it.
//!
//! Real generated code has one edge enum per handler; this machine shares
//! `Goto` (simple form, written in place) and two constructor-form edges
//! (`IdleToConnected`, `ConnectedToClosed`, take by value via `Slot`).

use estado_core::{Edge, Sink, Slot, Spec, StateId, StateKind};

use super::graph::{GRAPH, Leaf};
use super::{Conn, ConnCx, Connected, DrainSub, Draining, Idle, IdleSub, Keepalive, Link, Receipt};

pub fn note<S: Sink<()>>(cx: &mut ConnCx<'_, S>, line: String) {
    cx.ctx_mut().lines.push(line);
}

fn name(state: StateId) -> &'static str {
    GRAPH.state(state).name
}

/// Entry action of `state`. Entering a `#[final]` state inside a compound
/// state or region schedules a completion check (spec 5.10, 5.6 item 6); a
/// root `#[final]` has nothing to complete, it only makes `is_done()` true.
pub fn enter<S: Sink<()>>(state: StateId, cx: &mut ConnCx<'_, S>) {
    note(cx, format!("entry {}", name(state)));
    let info = GRAPH.state(state);
    if info.kind == StateKind::Final && info.parent.is_some() {
        cx.runtime_mut().completion_pending = true;
    }
}

pub fn exit<S: Sink<()>>(state: StateId, cx: &mut ConnCx<'_, S>) {
    note(cx, format!("exit {}", name(state)));
}

/// Entries of the whole active configuration: parents first, regions in
/// declaration order (spec 5.3 step 7).
pub fn enter_all<S: Sink<()>>(root: &mut Conn, cx: &mut ConnCx<'_, S>) {
    match root {
        Conn::Idle(idle) => {
            enter(Conn::IDLE, cx);
            enter(idle.sub.state_id(), cx);
        }
        Conn::Connected(connected) => {
            enter(Conn::CONNECTED, cx);
            enter(connected.link.state_id(), cx);
            enter(connected.keepalive.state_id(), cx);
        }
        Conn::Draining(draining) => {
            enter(Conn::DRAINING, cx);
            enter(draining.sub.state_id(), cx);
        }
        Conn::Closed(_) => enter(Conn::CLOSED, cx),
    }
}

/// Exits of the whole active configuration: deepest first, regions in
/// reverse declaration order (spec 5.3 step 1).
pub fn exit_all<S: Sink<()>>(root: &mut Conn, cx: &mut ConnCx<'_, S>) {
    match root {
        Conn::Idle(idle) => {
            exit(idle.sub.state_id(), cx);
            exit(Conn::IDLE, cx);
        }
        Conn::Connected(connected) => {
            exit(connected.keepalive.state_id(), cx);
            exit(connected.link.state_id(), cx);
            exit(Conn::CONNECTED, cx);
        }
        Conn::Draining(draining) => {
            exit(draining.sub.state_id(), cx);
            exit(Conn::DRAINING, cx);
        }
        Conn::Closed(_) => exit(Conn::CLOSED, cx),
    }
}

/// `reenter!()` of a top-level handler: exit and enter everything in place;
/// the state's own data stays (ADR 0006).
pub fn reenter_all<S: Sink<()>>(root: &mut Conn, cx: &mut ConnCx<'_, S>) {
    exit_all(root, cx);
    enter_all(root, cx);
}

/// `reenter!()` of a region leaf.
pub fn reenter_leaf<S: Sink<()>>(leaf: StateId, cx: &mut ConnCx<'_, S>) {
    exit(leaf, cx);
    enter(leaf, cx);
}

/// A transition inside one compound state or region: the LCA is the parent
/// of both leaves, so only the two leaves exit and enter.
fn switch_leaf<Level: Leaf, S: Sink<()>>(current: &mut Level, next: Level, cx: &mut ConnCx<'_, S>) {
    exit(current.state_id(), cx);
    *current = next;
    enter(current.state_id(), cx);
}

/// Simple-form edges: the target value is built in the decision phase and
/// written in place, so the machine is never poisoned.
pub enum Goto {
    /// Inside the `link` region.
    LinkTo(Link),
    /// Inside the `keepalive` region.
    KeepaliveTo(Keepalive),
    /// Inside `Idle`.
    IdleSubTo(IdleSub),
    /// Inside `Draining`.
    DrainSubTo(DrainSub),
    /// Top-level target `Idle`, from anywhere.
    Idle,
    /// Top-level target `Draining`, from anywhere.
    Draining,
}

impl Goto {
    /// Whether the LCA is above the parallel state `Connected`: the
    /// short-circuit of spec 5.6, item 3.
    pub fn leaves_parallel(&self) -> bool {
        matches!(self, Self::Idle | Self::Draining)
    }
}

impl Edge<Conn> for Goto {
    const TARGETS: &'static [StateId] = &[
        Conn::OPENING,
        Conn::OPEN,
        Conn::FINISHED,
        Conn::PINGING,
        Conn::AWAITING_PONG,
        Conn::DEAD,
        Conn::STOPPED,
        Conn::WAITING,
        Conn::READY,
        Conn::FLUSHING,
        Conn::FLUSHED,
        Conn::IDLE,
        Conn::DRAINING,
    ];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        let before = slot.live_mut().path();
        match (self, slot.live_mut()) {
            (Self::LinkTo(next), Conn::Connected(connected)) => {
                switch_leaf(&mut connected.link, next, cx);
            }
            (Self::KeepaliveTo(next), Conn::Connected(connected)) => {
                switch_leaf(&mut connected.keepalive, next, cx);
            }
            (Self::IdleSubTo(next), Conn::Idle(idle)) => switch_leaf(&mut idle.sub, next, cx),
            (Self::DrainSubTo(next), Conn::Draining(draining)) => {
                switch_leaf(&mut draining.sub, next, cx);
            }
            (Self::Idle, root) => replace_in_place(root, Conn::fresh_idle(), cx),
            (Self::Draining, root) => replace_in_place(root, Conn::fresh_draining(), cx),
            _ => unreachable!("edge applied from a state that cannot declare it"),
        }
        before != slot.live_mut().path()
    }
}

/// Steps 1, 6 and 7 of spec 5.3 for a simple-form edge; steps 2 and 3 have
/// nothing to do (no timers, no history), steps 4 and 5 do not exist.
fn replace_in_place<S: Sink<()>>(root: &mut Conn, next: Conn, cx: &mut ConnCx<'_, S>) {
    exit_all(root, cx);
    *root = next;
    enter_all(root, cx);
}

/// Steps 1 to 7 of spec 5.3 for a constructor-form edge: the old value is
/// taken by value (the slot is poisoned until `put`) and handed to
/// `construct`, which returns the new root.
fn replace_with<S: Sink<()>>(
    slot: &mut Slot<Conn>,
    cx: &mut ConnCx<'_, S>,
    construct: impl FnOnce(Conn) -> Conn,
) {
    exit_all(slot.live_mut(), cx);
    let old = slot.take();
    let new = construct(old);
    slot.put(new);
    enter_all(slot.live_mut(), cx);
}

/// `on_done(Idle)`: `goto!(Connected, |old: Idle| ..)`.
pub struct IdleToConnected<Build>(pub Build);

impl<Build: FnOnce(Idle) -> Connected> Edge<Conn> for IdleToConnected<Build> {
    const TARGETS: &'static [StateId] = &[Conn::CONNECTED];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        replace_with(slot, cx, |old| match old {
            Conn::Idle(idle) => Conn::Connected((self.0)(idle)),
            _ => unreachable!("only Idle declares this edge"),
        });
        true
    }
}

/// `on_done(Connected)`: `goto!(Closed, |old: Connected| ..)`.
pub struct ConnectedToClosed<Build>(pub Build);

impl<Build: FnOnce(Connected) -> Receipt> Edge<Conn> for ConnectedToClosed<Build> {
    const TARGETS: &'static [StateId] = &[Conn::CLOSED];

    fn apply<S: Sink<()>>(self, slot: &mut Slot<Conn>, cx: &mut ConnCx<'_, S>) -> bool {
        replace_with(slot, cx, |old| match old {
            Conn::Connected(connected) => Conn::Closed((self.0)(connected)),
            _ => unreachable!("only Connected declares this edge"),
        });
        true
    }
}

impl Conn {
    pub fn fresh_idle() -> Self {
        Self::Idle(Idle {
            attempts: 0,
            sub: IdleSub::Waiting,
        })
    }

    pub fn fresh_draining() -> Self {
        Self::Draining(Draining {
            sub: DrainSub::Flushing,
        })
    }
}
