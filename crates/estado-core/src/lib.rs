//! Sans-IO, deterministic core of `estado`: the [`Spec`] trait, the
//! run-to-completion [`Machine`], [`Next`] and typed [`Edge`]s, [`Cx`],
//! [`Effect`]s, sinks and the static [`Graph`].
//!
//! A complete hand-written machine, the code the `#[machine]` macro generates,
//! is shown on [`Machine`].
//!
//! Builds as `no_std` without `alloc` when the default `std` feature is off.
#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod context;
mod driver;
mod effect;
mod error;
mod graph;
mod ids;
mod machine;
mod next;
mod pending;
mod queue;
mod report;
mod runtime;
mod sink;
mod slot;
mod spec;

pub use context::{Cx, Fault};
pub use driver::{Driver, DriverSink, drive};
pub use effect::{ChildPath, Effect, InvokeKey, TimerKey};
pub use error::SendError;
pub use graph::{EdgeInfo, EdgeKind, EdgeTarget, Graph, HistoryKind, StateInfo, StateKind};
pub use ids::{MAX_ACTIVE_STATES, StateId, StatePath};
pub use machine::Machine;
pub use next::{Edge, Next, NoEdge};
pub use pending::PendingFlags;
pub use queue::{InlineQueue, Queues};
pub use report::{IgnoreReason, Outcome, Report, Step};
pub use runtime::{EventQueues, NoRuntime, Runtime};
pub use sink::{Discard, FnSink, InlineEffects, IntoEffects, Overflow, Sink};
pub use slot::Slot;
pub use spec::{DeferOverflowPolicy, Spec, UnhandledPolicy};
