//! Scenario I: the media player of the README and spec 3.2, written by hand
//! against `estado-core` exactly as `#[machine]` would generate it.
//!
//! What it stresses (spec 11, I):
//!
//! - nesting: `Player` > `Active` > `ActiveSub`, three levels;
//! - the `*Fields` split borrow: child handlers get `&mut` to their own data
//!   and, next to it, [`ActiveFields`], the fields of the ancestor `Active`
//!   without its `#[substates]` field (spec 6.2);
//! - views of struct-like variants (`PlayingMut`, `PausedMut`, `FailedMut`),
//!   decided in ADR 0007;
//! - a constructor-form edge that moves the non-`Clone` [`AudioBuffer`] out of
//!   `Active` into `Failed`, and back out of `Failed` into a new `Active`.
//!
//! Module map, one file per kind of generated code:
//!
//! | File | What the macro would generate there |
//! | --- | --- |
//! | `model` | the user's own types: states, events, commands |
//! | `views` | `*Fields`, `*Mut` views, `Exiting![..]` types |
//! | `graph` | state ids, `Graph`, `path()` |
//! | `actions` | entries and exits, linear code per level |
//! | `edges` | one edge enum per handler, with the transition code of 5.3 |
//! | `handlers` | the user's handlers, after `goto!` was rewritten |
//! | `dispatch` | the nested `match` and the `Spec` impl |

mod actions;
mod dispatch;
mod edges;
mod graph;
mod handlers;
mod model;
mod views;

pub use model::{Active, ActiveSub, AudioBuffer, Command, Event, Player, TrackId};
pub use views::{ActiveExiting, ActiveFields, FailedExiting, FailedMut, PausedMut, PlayingMut};
