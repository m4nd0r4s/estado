//! Sans-IO, deterministic core of `estado`: traits, the step algorithm,
//! `Next`, `Cx`, `Effect`, effect sinks and graph types.
//!
//! Builds as `no_std` without `alloc` when the default `std` feature is off.
#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;
