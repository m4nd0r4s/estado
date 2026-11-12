//! A crate that depends on `estado-core` under another name (`ec`) and on
//! `estado-macros`, without the `estado` facade.
//!
//! `#[machine]` must find the renamed core crate (ADR 0013 amendment 17): a
//! generated path that names `::estado_core` fails to compile here. The
//! machine is built into the library, so a plain `cargo build` guards it.
#![allow(missing_docs)]

use ec::Next;
use estado_macros::machine;

/// An event of the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Toggle {
    /// Flip the switch.
    Flip,
}

#[machine(event = Toggle)]
mod switch {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Switch {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Switch {
        #[on(Off)]
        fn off(_event: &Toggle) -> Next<Self> {
            goto!(On)
        }

        #[on(On)]
        fn on(_event: &Toggle) -> Next<Self> {
            goto!(Off)
        }
    }
}

pub use switch::Switch;
