//! Graph export (Mermaid, DOT, SCXML, JSON) for estado machines.
//!
//! Every `#[machine]` has a static [`Graph`](estado_core::Graph) (`GRAPH`, or
//! `M::GRAPH` through `Spec`). This crate turns it into text:
//!
//! | Format | Function | Method of [`GraphExport`] |
//! | --- | --- | --- |
//! | Mermaid `stateDiagram-v2` | [`to_mermaid`] | `to_mermaid()` |
//! | Graphviz DOT | [`to_dot`] | `to_dot()` |
//! | W3C SCXML | [`to_scxml`] | `to_scxml()` |
//! | JSON, see `docs/graph.schema.json` | [`to_json`] | `to_json()` |
//!
//! The output depends only on the graph: states and edges come out in
//! declaration order, so it is stable and fit for diffs and snapshots.
//! [`assert_snapshot!`] compares it with a committed file and rewrites the
//! file when `ESTADO_BLESS=1` (feature `std`, on by default).
//!
//! ```
//! use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind};
//! use estado_graph::GraphExport;
//!
//! static STATES: [StateInfo; 2] = [
//!     StateInfo::new(StateId::new(0), "Off", None, StateKind::Atomic, 0),
//!     StateInfo::new(StateId::new(1), "On", None, StateKind::Atomic, 0),
//! ];
//! static EDGES: [EdgeInfo; 2] = [
//!     EdgeInfo {
//!         from: StateId::new(0),
//!         to: EdgeTarget::State(StateId::new(1)),
//!         kind: EdgeKind::Normal,
//!         event_hint: Some("Event::Toggle"),
//!     },
//!     EdgeInfo {
//!         from: StateId::new(1),
//!         to: EdgeTarget::State(StateId::new(0)),
//!         kind: EdgeKind::Normal,
//!         event_hint: Some("Event::Toggle"),
//!     },
//! ];
//! static GRAPH: Graph = Graph { machine: "Light", states: &STATES, edges: &EDGES };
//!
//! let diagram = GRAPH.to_mermaid();
//! assert!(diagram.starts_with("stateDiagram-v2\n"));
//! assert!(diagram.contains("    [*] --> Off\n"));
//! assert!(diagram.contains("    Off --> On : Event#58;#58;Toggle\n"));
//! ```
//!
//! # Limits
//!
//! The graph does not record the `#[initial]` of the root enum, so every
//! format takes the first top-level state in declaration order as the initial
//! state of the machine (spec errata 19).
//!
//! Builds as `no_std` with `alloc`; the `std` feature adds the snapshot
//! helpers.
#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod dot;
mod format;
mod json;
mod label;
mod mermaid;
mod out;
mod scxml;
#[cfg(feature = "std")]
pub mod snapshot;
mod tree;

pub use dot::to_dot;
pub use format::{Format, GraphExport};
pub use json::{JSON_VERSION, to_json};
pub use mermaid::to_mermaid;
pub use scxml::to_scxml;
