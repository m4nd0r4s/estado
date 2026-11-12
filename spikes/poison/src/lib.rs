//! Spike for spec item 13.1: four ways to hide the poisoned state of a machine.
//!
//! * [`option_machine`]: `Option<M>` with `take()` (option b).
//! * [`slot_machine`]: option b generalised to `Unstarted | Live(M) | Poisoned`.
//! * [`mirror_machine`]: safe mirror enum with an extra variant (option a, safe form).
//! * [`mirror_cast_machine`]: mirror enum plus a `&Repr -> &M` cast (option a, unsafe form).
//! * [`flag_machine`]: `M` plus a `bool`, with a placeholder value while taken.
//! * [`uninit_machine`]: `MaybeUninit<M>` plus a `bool` and a drop guard.
//!
//! Only the modules that need `unsafe` allow it.
#![deny(unsafe_code)]

pub mod api;
pub mod fixtures_flat;
pub mod fixtures_full256;
pub mod fixtures_marked;
pub mod fixtures_player;
pub mod fixtures_tracked;
pub mod flag_machine;
pub mod mirror_cast_machine;
pub mod mirror_machine;
pub mod mirrors;
pub mod option_machine;
pub mod slot_machine;
#[allow(unsafe_code)]
pub mod uninit_machine;

pub use api::{MachineApi, Mirrored, Script, SendError, Spec, StateAccess};
