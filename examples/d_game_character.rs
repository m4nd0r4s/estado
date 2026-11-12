//! Scenario D: a game character with a parallel `Alive` state and a `Dead`
//! state, written by hand against `estado-core` exactly as `#[machine]` would
//! generate it.
//!
//! ```text
//! Alive (parallel, #[initial]; health, ammo)
//!   movement: Standing (#[initial]), Walking,
//!             Jumping (after 600 ms => Land), Crouching
//!   weapon:   Ready (#[initial]), Firing (after 120 ms => CooldownElapsed),
//!             Reloading (after 1.5 s => ReloadDone), Empty
//!   anim:     Idle (#[initial]), Run, Jump, Shoot
//! Dead
//! ```
//!
//! What it stresses (spec 11, D):
//!
//! - **high frequency**: events are `Copy` and four bytes wide, the context is
//!   one pointer ([`GameCtx`], no trace unless a test asks), the effects fit
//!   the inline buffer and nothing allocates on the hot path. It is the
//!   fixture BM-04 reuses (10,000 machines times 60 events);
//! - **`cx.in_state` across regions**: `weapon` refuses to reload while
//!   `movement` is `Jumping`, and `anim` derives its pose from both;
//! - **preemption**: a fatal `Damage` is handled by `Alive` itself, so
//!   `anim`, `weapon` and `movement` exit in that order, then `Alive`, and the
//!   reload (or jump, or cooldown) timer is cancelled with them (5.3);
//! - **defer**: `ReloadRequest` is deferred while airborne or mid-shot and
//!   redelivered when the configuration changes (5.5).
//!
//! Open item 13.4 (regions: sequential or all-see-event) is decided in
//! ADR 0008 and discussed at the top of the `dispatch` module.
//!
//! Module map, one file per kind of generated code:
//!
//! | File | What the macro would generate there |
//! | --- | --- |
//! | `model` | the user's own types: states, events, commands, constants |
//! | `ctx` | the user's global context |
//! | `views` | `AliveFields` |
//! | `graph` | state ids, `Graph`, `path()` |
//! | `runtime` | the defer queue and epoch counters |
//! | `tasks` | timer keys, start and cancel per region |
//! | `actions` | entries and exits, linear code per level |
//! | `edges`, `movement_edges`, `weapon_edges`, `anim_edges` | one edge enum per handler, with the transition code of 5.3 |
//! | `handlers`, `movement_handlers`, `weapon_handlers`, `anim_handlers` | the user's handlers, after `goto!` was rewritten |
//! | `dispatch` | decision phase, the three regions of `Alive` |
//! | `spec` | the `Spec` impl and `timer_event` |

mod actions;
mod anim_edges;
mod anim_handlers;
mod ctx;
mod dispatch;
mod edges;
mod graph;
mod handlers;
mod model;
mod movement_edges;
mod movement_handlers;
mod runtime;
mod spec;
mod tasks;
mod views;
mod weapon_edges;
mod weapon_handlers;

pub use ctx::GameCtx;
pub use model::{
    Alive, Anim, Character, Command, DEFER_CAP, DEFER_OVERFLOW_POLICY, Event, FIRE_COOLDOWN,
    JUMP_TIME, MAGAZINE_SIZE, MAX_HEALTH, Movement, RAISE_CAP, RELOAD_TIME, Sound, Weapon,
};
pub use runtime::CharacterRuntime;
pub use views::AliveFields;
