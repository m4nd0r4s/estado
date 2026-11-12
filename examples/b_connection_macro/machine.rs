//! The connection as a `#[machine]` module (ADR 0010).
//!
//! Compared with the hand-written scenario: the enums and structs are the
//! same; `#[handlers]` replaces the dispatch, the edge enums, the entries and
//! exits, the timers and invokes, the regions, the views and the graph, all
//! generated. The timers and invokes are declared as attributes on the
//! states; the regions are `#[region]` fields of `Connected`.

use estado_core::{Cx, Next, Sink};

use super::{
    CONNECT_TIMEOUT, Command, Event, Frame, MAX_ATTEMPTS, PING_INTERVAL, PONG_TIMEOUT, Session,
    backoff,
};

/// Appends a line to the trace kept in the global context.
fn note<S: Sink<Command>>(cx: &mut Cx<'_, conn::Conn, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

/// Traces and emits a command.
fn emit<S: Sink<Command>>(cx: &mut Cx<'_, conn::Conn, S>, command: Command) {
    let line = format!("emit {command:?}");
    cx.ctx_mut().push(line);
    cx.emit(command);
}

#[estado_macros::machine(event = Event, effect = Command, ctx = Vec<String>, defer_cap = 4)]
pub mod conn {
    use super::*;

    /// The machine: root enum.
    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Conn {
        /// `#[initial]`.
        #[initial]
        Disconnected,
        /// Dials and waits for the result, at most `CONNECT_TIMEOUT`.
        #[after(CONNECT_TIMEOUT => Event::ConnectTimeout)]
        #[invoke(|connecting| Command::Dial { attempt: *connecting.attempt })]
        Connecting {
            /// Failed dials so far in this cycle.
            attempt: u8,
        },
        /// Waits before the next attempt.
        #[after(|backing_off| backoff(*backing_off.attempt) => Event::Retry)]
        Backoff {
            /// Failed dials so far in this cycle.
            attempt: u8,
        },
        /// Parallel state with the regions `link` and `keepalive`.
        Connected(Connected),
    }

    /// Parallel state: shared data and two `#[region]`s, in evaluation order.
    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Connected {
        /// The session the dial produced.
        pub session: Session,
        /// Frames the link has delivered.
        pub frames_sent: u32,
        /// First region.
        #[region]
        pub link: Link,
        /// Second region.
        #[region]
        pub keepalive: Keepalive,
    }

    /// Region `link`: one frame in flight at a time.
    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Link {
        /// `#[initial]`.
        #[initial]
        Idle,
        /// A frame is being written.
        #[invoke(|sending| Command::SendFrame(sending.frame.clone()))]
        Sending {
            /// The frame in flight.
            frame: Frame,
        },
    }

    /// Region `keepalive`.
    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Keepalive {
        /// `#[initial]`: a quiet interval, then a ping.
        #[initial]
        #[after(PING_INTERVAL => Event::PingDue)]
        Pinging,
        /// The ping is out; the pong is due.
        #[after(PONG_TIMEOUT => Event::PongTimeout)]
        AwaitingPong,
        /// The peer is gone.
        Dead,
    }

    #[handlers]
    impl Conn {
        // Entries and exits, one per state.

        #[entry(Disconnected)]
        fn entry_disconnected(cx: &mut Cx<Self>) {
            note(cx, "entry Disconnected");
        }

        #[exit(Disconnected)]
        fn exit_disconnected(cx: &mut Cx<Self>) {
            note(cx, "exit Disconnected");
        }

        #[entry(Connecting)]
        fn entry_connecting(_connecting: ConnectingMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Connecting");
        }

        #[exit(Connecting)]
        fn exit_connecting(_connecting: ConnectingMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Connecting");
        }

        #[entry(Backoff)]
        fn entry_backoff(_backoff: BackoffMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Backoff");
        }

        #[exit(Backoff)]
        fn exit_backoff(_backoff: BackoffMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Backoff");
        }

        #[entry(Connected)]
        fn entry_connected(_up: ConnectedFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Connected");
        }

        /// Whatever the way out, the transport session is closed.
        #[exit(Connected)]
        fn exit_connected(up: ConnectedFields<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Connected");
            emit(
                cx,
                Command::Close {
                    session: up.session.id,
                },
            );
        }

        #[entry(Idle)]
        fn entry_idle(cx: &mut Cx<Self>) {
            note(cx, "entry Idle");
        }

        #[exit(Idle)]
        fn exit_idle(cx: &mut Cx<Self>) {
            note(cx, "exit Idle");
        }

        #[entry(Sending)]
        fn entry_sending(_sending: SendingMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "entry Sending");
        }

        #[exit(Sending)]
        fn exit_sending(_sending: SendingMut<'_>, cx: &mut Cx<Self>) {
            note(cx, "exit Sending");
        }

        #[entry(Pinging)]
        fn entry_pinging(cx: &mut Cx<Self>) {
            note(cx, "entry Pinging");
        }

        #[exit(Pinging)]
        fn exit_pinging(cx: &mut Cx<Self>) {
            note(cx, "exit Pinging");
        }

        /// The ping goes out when the state is entered; the timer waits for
        /// the pong.
        #[entry(AwaitingPong)]
        fn entry_awaiting_pong(cx: &mut Cx<Self>) {
            note(cx, "entry AwaitingPong");
            emit(cx, Command::Ping);
        }

        #[exit(AwaitingPong)]
        fn exit_awaiting_pong(cx: &mut Cx<Self>) {
            note(cx, "exit AwaitingPong");
        }

        /// `Dead` lives for one microstep: its entry raises the event its own
        /// handler answers by leaving `Connected` (entries cannot transition,
        /// they may `raise`).
        #[entry(Dead)]
        fn entry_dead(cx: &mut Cx<Self>) {
            note(cx, "entry Dead");
            cx.raise(Event::KeepaliveDead);
        }

        #[exit(Dead)]
        fn exit_dead(cx: &mut Cx<Self>) {
            note(cx, "exit Dead");
        }

        // Event handlers.

        /// `Send` is not handled here: a frame sent while disconnected is
        /// ignored under the machine's `ignore` policy.
        #[on(Disconnected)]
        fn on_disconnected(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Disconnected");
            match event {
                Event::Connect => goto!(Connecting { attempt: 0 }),
                _ => bubble!(),
            }
        }

        /// Defers `Send` until there is a session.
        #[on(Connecting)]
        fn on_connecting(
            connecting: ConnectingMut<'_>,
            cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            note(cx, "handler Connecting");
            match event {
                Event::Connected(session) => goto!(Connected {
                    session: *session,
                    frames_sent: 0,
                }),
                Event::ConnectTimeout if *connecting.attempt + 1 >= MAX_ATTEMPTS => {
                    emit(cx, Command::GiveUp);
                    goto!(Disconnected)
                }
                Event::ConnectTimeout => goto!(Backoff {
                    attempt: *connecting.attempt,
                }),
                Event::Disconnect => goto!(Disconnected),
                Event::Send(_) => defer!(),
                _ => bubble!(),
            }
        }

        /// Defers `Send` too.
        #[on(Backoff)]
        fn on_backoff(backoff: BackoffMut<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Backoff");
            match event {
                Event::Retry => goto!(Connecting {
                    attempt: *backoff.attempt + 1,
                }),
                Event::Disconnect => goto!(Disconnected),
                Event::Send(_) => defer!(),
                _ => bubble!(),
            }
        }

        /// Reached only when no region consumed or deferred the event, so
        /// `Disconnect` lands here.
        #[on(Connected)]
        fn on_connected(up: ConnectedFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(
                cx,
                &format!("handler Connected (session {})", up.session.id),
            );
            match event {
                Event::Disconnect => goto!(Disconnected),
                _ => bubble!(),
            }
        }

        /// A leaf of `link`. Refuses to start a frame while the `keepalive`
        /// region waits for a pong (it asks `cx.in_state`), by deferring it:
        /// the pong changes the configuration, which redelivers the frame.
        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Idle");
            match event {
                Event::Send(_) if cx.in_state(Self::AWAITING_PONG) => {
                    note(cx, "link refuses to send while keepalive awaits the pong");
                    defer!()
                }
                Event::Send(frame) => goto!(Sending {
                    frame: frame.clone(),
                }),
                _ => bubble!(),
            }
        }

        /// One frame in flight, the next `Send` waits.
        #[on(Sending)]
        fn on_sending(
            _sending: SendingMut<'_>,
            up: ConnectedFields<'_>,
            cx: &mut Cx<Self>,
            event: &Event,
        ) -> Next<Self> {
            note(cx, "handler Sending");
            match event {
                Event::FrameSent => {
                    *up.frames_sent += 1;
                    goto!(Idle)
                }
                Event::Send(_) => defer!(),
                _ => bubble!(),
            }
        }

        /// A leaf of `keepalive`. A delivered frame proves the peer is alive,
        /// so `FrameSent` re-enters `Pinging`: a new epoch, and the ping
        /// interval starts over.
        #[on(Pinging)]
        fn on_pinging(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Pinging");
            match event {
                Event::PingDue => goto!(AwaitingPong),
                Event::FrameSent => reenter!(),
                _ => bubble!(),
            }
        }

        #[on(AwaitingPong)]
        fn on_awaiting_pong(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler AwaitingPong");
            match event {
                Event::Pong => goto!(Pinging),
                Event::PongTimeout => goto!(Dead),
                _ => bubble!(),
            }
        }

        /// Leaves `Connected`, so every region exits.
        #[on(Dead)]
        fn on_dead(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "handler Dead");
            match event {
                Event::KeepaliveDead => goto!(Backoff { attempt: 0 }),
                _ => bubble!(),
            }
        }
    }
}
