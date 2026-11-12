//! Spike for risk R1 (spec 4.2, 13.2): handlers return
//! `Next<impl Edge<M>>` where the hidden type is a per-handler edge enum,
//! generic over the types of the constructor closures.
//!
//! The code here is what `#[handlers]` would generate, written by hand.
//! `tests/` checks behaviour; `src/compile_fail.rs` holds the cases that must
//! not compile.

pub mod compile_fail;
pub mod core;
pub mod dispatch;
pub mod edges;
pub mod handlers;
pub mod model;
