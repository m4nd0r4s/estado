//! State id constants for regions, naming and attributes that only the
//! parser reads (they are stripped from the emitted types).
#![allow(dead_code, missing_docs, unused_imports)]

use std::time::Duration;

use estado_core::StateId;
use estado_macros::machine;

pub struct Ev;
pub struct Cmd;

#[machine(event = Ev, effect = Cmd, unhandled = error, version = 2)]
mod connection {
    use super::*;

    #[root]
    pub enum Connection {
        #[initial]
        Disconnected,
        #[after(Duration::from_secs(5) => Ev)]
        Connecting,
        Connected(Connected),
        #[terminal]
        Closed,
    }

    #[state]
    pub struct Connected {
        session: u32,
        #[region]
        link: Link,
        #[region]
        keep_alive: KeepAlive,
    }

    #[state]
    pub enum Link {
        #[initial]
        Open,
        Draining,
    }

    #[state]
    pub enum KeepAlive {
        #[initial(with = KeepAlive::new_waiting)]
        WaitingForPing { missed: u8 },
        #[unhandled(panic)]
        #[invoke(|state: &PingSent| Cmd)]
        PingSent,
    }

    impl KeepAlive {
        fn new_waiting() -> Self {
            Self::WaitingForPing { missed: 0 }
        }
    }
}

pub use connection::Connection;

#[test]
fn regions_and_acronym_names_get_constants() {
    assert_eq!(Connection::DISCONNECTED, StateId::new(0));
    assert_eq!(Connection::CONNECTING, StateId::new(1));
    assert_eq!(Connection::CONNECTED, StateId::new(2));
    assert_eq!(Connection::LINK, StateId::new(3));
    assert_eq!(Connection::OPEN, StateId::new(4));
    assert_eq!(Connection::DRAINING, StateId::new(5));
    assert_eq!(Connection::KEEP_ALIVE, StateId::new(6));
    assert_eq!(Connection::WAITING_FOR_PING, StateId::new(7));
    assert_eq!(Connection::PING_SENT, StateId::new(8));
    assert_eq!(Connection::CLOSED, StateId::new(9));
}
