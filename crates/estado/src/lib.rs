//! Hierarchical statecharts for Rust.
//!
//! Facade that re-exports [`estado_core`] and the macros from `estado-macros`.
#![no_std]

#[allow(unused_imports)] // empty until M2
pub use estado_core::*;
#[allow(unused_imports)] // empty until M3
pub use estado_macros::*;

/// Common imports.
pub mod prelude {}
