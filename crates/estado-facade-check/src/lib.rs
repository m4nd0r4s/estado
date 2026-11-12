//! A crate that depends on the `estado` facade and on nothing else.
//!
//! `#[machine]` must generate code that resolves through the facade alone
//! (ADR 0013, item 1): this crate has no `estado-core` or `estado-macros` in
//! its dependencies, so any generated path that names `::estado_core`
//! directly fails to compile here. The machine below is built into the
//! library, so a plain `cargo build` guards the paths; `tests/facade_only.rs`
//! adds regions and a completion.
#![allow(missing_docs)]

use estado::prelude::*;

/// An event of the library machine.
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
        fn off(event: &Toggle) -> Next<Self> {
            match event {
                Toggle::Flip => goto!(On),
            }
        }

        #[on(On)]
        fn on(event: &Toggle) -> Next<Self> {
            match event {
                Toggle::Flip => goto!(Off),
            }
        }
    }
}

pub use switch::Switch;
