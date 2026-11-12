//! Hierarchical statecharts for Rust.
//!
//! Facade that re-exports [`estado_core`] and the macros from `estado-macros`.
//! Start from the [`prelude`]:
//!
//! ```ignore
//! use estado::prelude::*;
//!
//! #[machine(event = Ev)]
//! mod light {
//!     #[root]
//!     pub enum Light {
//!         #[initial]
//!         Off,
//!         On,
//!     }
//!
//!     #[handlers]
//!     impl Light {
//!         #[on(Off)]
//!         fn off(event: &Ev) -> Next<Self> {
//!             match event {
//!                 Ev::Toggle => goto!(On),
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! The transition macros ([`goto!`], [`stay!`], ..) are only meaningful inside
//! a `#[handlers]` function of a `#[machine]` module, which rewrites them
//! before they expand. The definitions in this crate exist so the names
//! resolve and so a use anywhere else reports error `E0040`.
//!
//! Builds as `no_std` without `alloc` when the default `std` feature is off.
#![no_std]

pub use estado_core::*;
pub use estado_macros::{handlers, machine, root, state};

mod transitions;

/// Not public API: the generated code of `#[machine]` reaches `estado-core`
/// through here, so a crate that depends on `estado` alone can use it.
#[doc(hidden)]
pub mod __private {
    pub use estado_core as core;
}

/// Common imports: `use estado::prelude::*;`.
pub mod prelude {
    pub use estado_core::{
        Cx, Discard, Edge, Effect, Machine, Next, Report, SendError, Sink, Spec, StateId, Step,
    };
    pub use estado_macros::{handlers, machine, root, state};

    pub use crate::{Exiting, bubble, defer, edges, goto, goto_dyn, reenter, stay, unhandled};
}

/// The README's examples, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
mod readme {}
