//! A `pub` machine that uses timers, tasks, regions, history, completion,
//! `#[always]`, the queues and `in_state`, in a crate that denies
//! `missing_docs` and `unreachable_pub`: nothing generated may trip them.
#![deny(missing_docs, unreachable_pub)]

use std::time::Duration;

use estado_core::{Cx, Next};
use estado_macros::machine;

/// Events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ev;

/// Effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd;

/// The machine.
#[machine(event = Ev, effect = Cmd, unhandled = error, allow(W0001, W0002))]
pub mod connection {
    use super::*;

    /// The root.
    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Connection {
        /// Not connected.
        #[initial]
        Disconnected,
        /// Dialling.
        #[after(Duration::from_secs(5) => Ev)]
        Connecting,
        /// Connected.
        Connected(Connected),
        /// A session with a memory.
        Session(Session),
        /// Done.
        #[terminal]
        Closed,
    }

    /// A compound state with shallow history.
    #[state]
    #[history]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Session {
        /// Its mode.
        #[substates]
        pub mode: Mode,
    }

    /// The modes.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        /// Calm.
        #[initial]
        Calm,
        /// Busy.
        Busy,
    }

    /// The connected state.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Connected {
        /// The session.
        pub session: u32,
        /// The link.
        #[region]
        pub link: Link,
        /// The keep-alive.
        #[region]
        pub keep_alive: KeepAlive,
    }

    /// The link.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Link {
        /// Open.
        #[initial]
        Open,
        /// Draining.
        Draining,
        /// Finished.
        #[terminal]
        Shut,
    }

    /// The keep-alive.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum KeepAlive {
        /// Waiting.
        #[initial]
        Waiting,
        /// A ping is out.
        #[invoke(|_: &()| Cmd)]
        PingSent,
        /// Finished.
        #[terminal]
        Silent,
    }

    #[handlers]
    impl Connection {
        /// Starts.
        #[on(Disconnected)]
        fn disconnected(cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            cx.raise(Ev);
            goto!(Connecting)
        }

        /// Connects.
        #[on(Connecting)]
        fn connecting(_event: &Ev) -> Next<Self> {
            goto!(Connected { session: 1 })
        }

        /// Returns to where the session was.
        #[on(Busy)]
        fn busy(_event: &Ev) -> Next<Self> {
            goto!(Session, history)
        }

        /// Defers.
        #[on(Open)]
        fn open(cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            if cx.in_state(Connection::OPEN_ID) {
                defer!()
            } else {
                stay!()
            }
        }

        /// Both regions are done.
        #[on_done(Connected)]
        fn connected_done(_cx: &mut Cx<Self>) -> Next<Self> {
            goto!(Closed)
        }

        /// Pings.
        #[always(Waiting)]
        fn waiting(_cx: &mut Cx<Self>) -> Next<Self> {
            stay!()
        }
    }
}

fn main() {}
