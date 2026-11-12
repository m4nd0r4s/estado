//! Cases that must not compile, as `compile_fail` doctests.
//!
//! # Without precise capturing the edge keeps the state borrowed
//!
//! Edition 2024 return-position `impl Trait` captures every lifetime in
//! scope, so the opaque edge type borrows `position` and the transition
//! cannot write the state while it lives:
//!
//! ```compile_fail,E0506
//! use typed_edges_spike::core::{Context, Edge, Next};
//! use typed_edges_spike::edges::PlayingEdges;
//! use typed_edges_spike::model::{Active, Player};
//!
//! fn handler(position: &mut u32) -> Next<impl Edge<Player>> {
//!     Next::Goto(PlayingEdges::<fn(Active) -> Player, fn(Active) -> Player, _>::ToFailed2(
//!         |old: Active| Player::Failed { reason: String::new(), buffer: old.buffer },
//!     ))
//! }
//!
//! fn microstep(position: &mut u32, machine: &mut Player) {
//!     let next = handler(position);
//!     *position = 0; // the state is written while `next` is alive
//!     if let Next::Goto(edge) = next {
//!         edge.apply(machine, &mut Context::default());
//!     }
//! }
//! ```
//!
//! # A constructor cannot capture the state it is about to replace
//!
//! With `use<'event>`, capturing the state data is rejected in the handler,
//! with an error that points at the closure (E0700; its `help` suggests
//! widening `use<..>`, which is the wrong fix: see ADR 0001):
//!
//! ```compile_fail,E0700
//! use typed_edges_spike::core::{Edge, Next};
//! use typed_edges_spike::edges::PlayingEdges;
//! use typed_edges_spike::model::{Active, Player};
//!
//! fn handler<'event>(position: &mut u32, _event: &'event ()) -> Next<impl Edge<Player> + use<'event>> {
//!     Next::Goto(PlayingEdges::<fn(Active) -> Player, fn(Active) -> Player, _>::ToFailed2(
//!         move |old: Active| {
//!             *position += 1;
//!             Player::Failed { reason: String::new(), buffer: old.buffer }
//!         },
//!     ))
//! }
//! ```
//!
//! # Two `goto!` cannot share one generic variant
//!
//! Every closure has its own type, so one variant per target would not
//! unify. This is why the macro generates one variant per occurrence:
//!
//! ```compile_fail,E0308
//! use typed_edges_spike::core::{Edge, Next};
//! use typed_edges_spike::edges::PlayingEdges;
//! use typed_edges_spike::model::{Active, Player};
//!
//! fn handler(flag: bool) -> Next<impl Edge<Player>> {
//!     if flag {
//!         return Next::Goto(PlayingEdges::<_, fn(Active) -> Player, fn(Active) -> Player>::ToFailed0(
//!             |old: Active| Player::Failed { reason: "a".into(), buffer: old.buffer },
//!         ));
//!     }
//!     Next::Goto(PlayingEdges::ToFailed0(
//!         |old: Active| Player::Failed { reason: "b".into(), buffer: old.buffer },
//!     ))
//! }
//! ```
