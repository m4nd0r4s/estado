//! `Conn`: scenario B of spec 11 (connection with a parallel `Connected`),
//! written by hand against `Spec` as the macro would generate it.
//!
//! ```text
//! Idle (compound)                    on_done -> Connected
//!   Waiting, Ready (#[final], u32)
//! Connected (parallel, session data)  on_done -> Closed
//!   link:      Opening, Open, Finished (#[final])
//!   keepalive: Pinging, AwaitingPong, Dead, Stopped (#[final])
//! Draining (compound, no on_done)
//!   Flushing, Flushed (#[final])
//! Closed(Receipt) (root #[final], output)
//! ```
//!
//! Every entry, exit, handler call and `on_done` is logged in the context so
//! tests assert exact orders. `TRACKS_ACTIVE` is on, so handlers can ask
//! `cx.in_state(..)` about sibling regions.

mod dispatch;
mod edges;
mod graph;
mod handlers;
pub mod testing;

use estado_core::{
    Cx, Graph, InlineEffects, Next, Outcome, Queues, Runtime, Sink, Slot, Spec, StateId, StatePath,
    UnhandledPolicy,
};

use dispatch::{dispatch_connected, resolve};
use edges::enter_all;
use graph::{GRAPH, Leaf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conn {
    Idle(Idle),
    Connected(Connected),
    Draining(Draining),
    /// `#[final]` with an output.
    Closed(Receipt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idle {
    pub attempts: u8,
    pub sub: IdleSub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleSub {
    Waiting,
    /// `#[final]`; the payload is the output given to `on_done(Idle)`.
    Ready(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connected {
    pub session: u32,
    pub frames: u32,
    pub resumed_after: u8,
    /// First region.
    pub link: Link,
    /// Second region.
    pub keepalive: Keepalive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Opening,
    Open,
    /// `#[final]`
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keepalive {
    Pinging,
    AwaitingPong,
    Dead,
    /// `#[final]`
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draining {
    pub sub: DrainSub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainSub {
    Flushing,
    /// `#[final]`
    Flushed,
}

/// Output of the root `#[final]`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub session: u32,
    pub frames: u32,
}

/// `IdleFields`: the fields of `Idle` without `#[substates]`.
pub struct IdleFields<'data> {
    pub attempts: &'data mut u8,
}

/// `ConnectedFields`: the fields of `Connected` without `#[region]`s.
pub struct ConnectedFields<'data> {
    pub session: &'data mut u32,
    pub frames: &'data mut u32,
    pub resumed_after: &'data mut u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `Idle`/`Waiting`: becomes `Ready(session)`.
    Ring(u32),
    /// `Waiting`: counts an attempt in the fields of `Idle`.
    Retry,
    LinkUp,
    /// `Opening` goes `Open`; `Pinging` reads `in_state(Open)`.
    Sync,
    /// `Opening` stays; `Open` and `Pinging` leave `Connected`.
    Hangup,
    /// `Dead` leaves `Connected`; `Draining` leaves `Draining`.
    Reset,
    /// Only `Connected` itself handles it.
    Abort,
    Timeout,
    /// `Opening` and `Pinging` defer; `Open` stays.
    Hold,
    /// Only `Connected` itself handles it.
    Poke,
    /// Both regions go final.
    Close,
    /// Only the link goes final.
    CloseLink,
    /// `Open` re-enters itself.
    Refresh,
    Drain,
    Flush,
}

#[derive(Debug, Default)]
pub struct Log {
    pub lines: Vec<String>,
}

pub struct ConnRuntime {
    pub queues: Queues<Event, 4, 4>,
    /// Set by the entry of a `#[final]` state: some completion may be due.
    completion_pending: bool,
}

impl Runtime for ConnRuntime {
    type Event = Event;
    type Queues = Queues<Event, 4, 4>;
    const NEW: Self = Self {
        queues: Queues::new(),
        completion_pending: false,
    };

    fn queues(&mut self) -> &mut Self::Queues {
        &mut self.queues
    }

    /// Completion microsteps travel through the `#[always]` channel: they run
    /// before internal and requeued events (spec 5.4, ADR 0005).
    fn take_always_pending(&mut self) -> bool {
        core::mem::take(&mut self.completion_pending)
    }
}

pub type ConnCx<'step, S> = Cx<'step, Conn, S>;

impl Spec for Conn {
    type Event = Event;
    type Effect = ();
    type Ctx = Log;
    type Output = Receipt;
    type Runtime = ConnRuntime;
    type Effects = InlineEffects<(), 4>;

    const NAME: &'static str = "Conn";
    const GRAPH: &'static Graph = &GRAPH;
    const UNHANDLED: UnhandledPolicy = UnhandledPolicy::Ignore;
    const TRACKS_ACTIVE: bool = true;

    fn initial() -> Self {
        Self::fresh_idle()
    }

    fn state_id(&self) -> StateId {
        match self {
            Self::Idle(idle) => idle.sub.state_id(),
            Self::Connected(_) => Self::CONNECTED,
            Self::Draining(draining) => draining.sub.state_id(),
            Self::Closed(_) => Self::CLOSED,
        }
    }

    /// Pre-order: parent, then each region followed by its leaf.
    fn path(&self) -> StatePath {
        let mut path = StatePath::new(&GRAPH);
        match self {
            Self::Idle(idle) => {
                path.push(Self::IDLE);
                path.push(idle.sub.state_id());
            }
            Self::Connected(connected) => {
                path.push(Self::CONNECTED);
                path.push(Self::LINK);
                path.push(connected.link.state_id());
                path.push(Self::KEEPALIVE);
                path.push(connected.keepalive.state_id());
            }
            Self::Draining(draining) => {
                path.push(Self::DRAINING);
                path.push(draining.sub.state_id());
            }
            Self::Closed(_) => path.push(Self::CLOSED),
        }
        path
    }

    fn enter_initial<S: Sink<()>>(&mut self, cx: &mut ConnCx<'_, S>) {
        enter_all(self, cx);
    }

    fn microstep<S: Sink<()>>(
        slot: &mut Slot<Self>,
        cx: &mut ConnCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        let next = match slot.live_mut() {
            Self::Idle(Idle { attempts, sub }) => match sub {
                IdleSub::Waiting => handlers::waiting(IdleFields { attempts }, cx, event),
                IdleSub::Ready(_) => Next::Bubble,
            },
            Self::Connected(_) => return dispatch_connected(slot, cx, event),
            Self::Draining(draining) => {
                let leaf = match draining.sub {
                    DrainSub::Flushing => handlers::flushing(cx, event),
                    DrainSub::Flushed => Next::Bubble,
                };
                match leaf {
                    Next::Bubble => handlers::draining(cx, event),
                    handled => handled,
                }
            }
            Self::Closed(_) => Next::Unhandled,
        };
        resolve(next, slot, cx)
    }

    /// The synthetic completion microstep of spec 5.10 and 5.6 item 6.
    fn run_always<S: Sink<()>>(slot: &mut Slot<Self>, cx: &mut ConnCx<'_, S>) -> Outcome {
        match slot.live_mut() {
            Self::Idle(Idle {
                attempts,
                sub: IdleSub::Ready(session),
            }) => {
                let next = handlers::on_done_idle(IdleFields { attempts }, session, cx);
                resolve(next, slot, cx)
            }
            Self::Connected(Connected {
                session,
                frames,
                resumed_after,
                link: Link::Finished,
                keepalive: Keepalive::Stopped,
            }) => {
                let up = ConnectedFields {
                    session,
                    frames,
                    resumed_after,
                };
                let next = handlers::on_done_connected(up, cx);
                resolve(next, slot, cx)
            }
            // Not complete (one region still running), or no `on_done`.
            _ => Outcome::Stayed,
        }
    }

    fn is_done(&self) -> bool {
        matches!(self, Self::Closed(_))
    }

    /// Leaves the default receipt behind (ADR 0005): a second call yields it.
    fn take_output(&mut self) -> Option<Receipt> {
        match self {
            Self::Closed(receipt) => Some(core::mem::take(receipt)),
            _ => None,
        }
    }
}
