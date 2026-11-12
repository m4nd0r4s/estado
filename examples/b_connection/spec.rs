//! The `Spec` impl: what the core needs from the generated code (spec 4.1,
//! 7.1, 7.2).

use estado_core::{
    DeferOverflowPolicy, Graph, InlineEffects, InvokeKey, Outcome, Sink, Slot, Spec, StateId,
    StatePath, TimerKey, UnhandledPolicy,
};

use super::actions::{ConnCx, enter_root};
use super::dispatch::dispatch;
use super::graph::GRAPH;
use super::model::{Command, Conn, DEFER_OVERFLOW_POLICY, Event, Keepalive, Link};
use super::runtime::ConnRuntime;
use super::tasks::{
    backoff_timer, connect_timer, dial_invoke, ping_timer, pong_timer, send_invoke,
};

impl Spec for Conn {
    type Event = Event;
    type Effect = Command;
    type Ctx = Vec<String>;
    type Output = ();
    type Runtime = ConnRuntime;
    type Effects = InlineEffects<Command, 4>;

    const NAME: &'static str = "Conn";
    const GRAPH: &'static Graph = &GRAPH;
    const UNHANDLED: UnhandledPolicy = UnhandledPolicy::Ignore;
    const DEFER_OVERFLOW: DeferOverflowPolicy = DEFER_OVERFLOW_POLICY;
    /// `link` reads `keepalive` through `cx.in_state`.
    const TRACKS_ACTIVE: bool = true;

    fn initial() -> Self {
        Self::Disconnected
    }

    fn state_id(&self) -> StateId {
        match self {
            Self::Disconnected => Self::DISCONNECTED,
            Self::Connecting { .. } => Self::CONNECTING,
            Self::Backoff { .. } => Self::BACKOFF,
            Self::Connected(_) => Self::CONNECTED,
        }
    }

    fn path(&self) -> StatePath {
        self.active_path()
    }

    fn enter_initial<S: Sink<Command>>(&mut self, cx: &mut ConnCx<'_, S>) {
        enter_root(self, cx);
    }

    fn microstep<S: Sink<Command>>(
        slot: &mut Slot<Self>,
        cx: &mut ConnCx<'_, S>,
        event: &Event,
    ) -> Outcome {
        dispatch(slot, cx, event)
    }

    /// The event of a timer, built when it fires from the data of the state
    /// (spec 7.1). `None` unless the key names the active instance: right
    /// state and current epoch.
    fn timer_event(&self, runtime: &ConnRuntime, key: TimerKey) -> Option<Event> {
        match self {
            Self::Disconnected => None,
            Self::Connecting { .. } => {
                (key == connect_timer(runtime.connecting_epoch)).then_some(Event::ConnectTimeout)
            }
            Self::Backoff { .. } => {
                (key == backoff_timer(runtime.backoff_epoch)).then_some(Event::Retry)
            }
            Self::Connected(connected) => match connected.keepalive {
                Keepalive::Pinging if key == ping_timer(runtime.pinging_epoch) => {
                    Some(Event::PingDue)
                }
                Keepalive::AwaitingPong if key == pong_timer(runtime.awaiting_pong_epoch) => {
                    Some(Event::PongTimeout)
                }
                _ => None,
            },
        }
    }

    /// Whether the invoke belongs to the active instance (spec 7.2).
    fn accepts_invoke(&self, runtime: &ConnRuntime, key: InvokeKey) -> bool {
        match self {
            Self::Connecting { .. } => key == dial_invoke(runtime.connecting_epoch),
            Self::Connected(connected) => {
                matches!(connected.link, Link::Sending { .. })
                    && key == send_invoke(runtime.sending_epoch)
            }
            Self::Disconnected | Self::Backoff { .. } => false,
        }
    }
}
