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

#[cfg(test)]
mod own_machine_tests;

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

/// The user guide's examples (`docs/guide/`), compiled and run as doctests.
#[cfg(doctest)]
mod guide {
    #[doc = include_str!("../../../docs/guide/01-getting-started.md")]
    mod getting_started {}

    #[doc = include_str!("../../../docs/guide/02-states.md")]
    mod states {}

    #[doc = include_str!("../../../docs/guide/03-state-data.md")]
    mod state_data {}

    #[doc = include_str!("../../../docs/guide/04-handlers.md")]
    mod handlers {}

    #[doc = include_str!("../../../docs/guide/05-transitions.md")]
    mod transitions {}

    #[doc = include_str!("../../../docs/guide/06-transition-kinds.md")]
    mod transition_kinds {}

    #[doc = include_str!("../../../docs/guide/07-unhandled-events.md")]
    mod unhandled_events {}

    #[doc = include_str!("../../../docs/guide/08-entry-exit.md")]
    mod entry_exit {}

    #[doc = include_str!("../../../docs/guide/09-always.md")]
    mod always {}

    #[doc = include_str!("../../../docs/guide/10-on-done.md")]
    mod on_done {}

    #[doc = include_str!("../../../docs/guide/11-context-and-effects.md")]
    mod context_and_effects {}

    #[doc = include_str!("../../../docs/guide/12-sinks-and-drivers.md")]
    mod sinks_and_drivers {}

    #[doc = include_str!("../../../docs/guide/13-regions.md")]
    mod regions {}

    #[doc = include_str!("../../../docs/guide/14-region-coordination.md")]
    mod region_coordination {}

    #[doc = include_str!("../../../docs/guide/15-timers.md")]
    mod timers {}

    #[doc = include_str!("../../../docs/guide/16-tasks.md")]
    mod tasks {}

    #[doc = include_str!("../../../docs/guide/17-history.md")]
    mod history {}

    #[doc = include_str!("../../../docs/guide/18-defer-and-raise.md")]
    mod defer_and_raise {}

    #[doc = include_str!("../../../docs/guide/19-testing.md")]
    mod testing {}

    #[doc = include_str!("../../../docs/guide/20-testing-tasks-and-errors.md")]
    mod testing_tasks_and_errors {}

    #[doc = include_str!("../../../docs/guide/21-errors-and-safety.md")]
    mod errors_and_safety {}

    #[doc = include_str!("../../../docs/guide/22-warnings-and-options.md")]
    mod warnings_and_options {}

    #[doc = include_str!("../../../docs/guide/23-no-std.md")]
    mod no_std {}
}
