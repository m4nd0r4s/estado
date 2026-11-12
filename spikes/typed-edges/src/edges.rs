//! Edge enums the macro generates, one per handler, one variant per `goto!`.
//!
//! Constructor closures make a variant generic. The closure type is named
//! only by the generic parameter: the handler never spells it, inference
//! fills it from the branch that builds the variant, and every other branch
//! unifies with the same opaque return type.
//!
//! `apply` holds the linear transition code of spec 5.3 for each edge.

use crate::core::{Context, Edge};
use crate::model::{Active, ActiveSub, Player};

pub enum PlayingEdges<FailedConstructor0, FailedConstructor1, FailedConstructor2> {
    /// `goto!(Paused { position })`, simple form, local to `Active`.
    ToPaused {
        position: u32,
    },
    /// `goto!(Idle)`, LCA at the root.
    ToIdle,
    ToFailed0(FailedConstructor0),
    ToFailed1(FailedConstructor1),
    ToFailed2(FailedConstructor2),
}

impl<FailedConstructor0, FailedConstructor1, FailedConstructor2> Edge<Player>
    for PlayingEdges<FailedConstructor0, FailedConstructor1, FailedConstructor2>
where
    FailedConstructor0: FnOnce(Active) -> Player,
    FailedConstructor1: FnOnce(Active) -> Player,
    FailedConstructor2: FnOnce(Active) -> Player,
{
    const TARGETS: &'static [&'static str] = &["Paused", "Idle", "Failed", "Failed", "Failed"];

    fn apply(self, machine: &mut Player, context: &mut Context) {
        match self {
            Self::ToPaused { position } => {
                let Player::Active(active) = machine else {
                    unreachable!("Playing is active")
                };
                context.note("exit Playing");
                // Take, construct, write on the LCA child (`Active.sub`).
                active.sub = ActiveSub::Paused { position };
                context.note("entry Paused");
            }
            Self::ToIdle => {
                exit_playing_and_active(context);
                *machine = Player::Idle;
                context.note("entry Idle");
            }
            Self::ToFailed0(constructor) => leave_active_into(machine, context, constructor),
            Self::ToFailed1(constructor) => leave_active_into(machine, context, constructor),
            Self::ToFailed2(constructor) => leave_active_into(machine, context, constructor),
        }
    }
}

pub enum ActiveEdges {
    ToPlaying {
        position: u32,
    },
    /// `goto!(Active, external)`: exit and re-enter `Active` itself.
    ReenterActive,
}

impl Edge<Player> for ActiveEdges {
    const TARGETS: &'static [&'static str] = &["Playing", "Active"];

    fn apply(self, machine: &mut Player, context: &mut Context) {
        let Player::Active(active) = machine else {
            unreachable!("Active is active")
        };
        match self {
            Self::ToPlaying { position } => {
                context.note(match active.sub {
                    ActiveSub::Playing { .. } => "exit Playing",
                    ActiveSub::Paused { .. } => "exit Paused",
                });
                active.sub = ActiveSub::Playing { position };
                context.note("entry Playing");
            }
            Self::ReenterActive => {
                context.note(match active.sub {
                    ActiveSub::Playing { .. } => "exit Playing",
                    ActiveSub::Paused { .. } => "exit Paused",
                });
                context.note("exit Active");
                active.sub = ActiveSub::Playing { position: 0 };
                context.note("entry Active");
                context.note("entry Playing");
            }
        }
    }
}

fn exit_playing_and_active(context: &mut Context) {
    context.note("exit Playing");
    context.note("exit Active");
}

/// Exits down to the root, takes `Active` by value, runs the constructor
/// (the UML transition action) and writes the result.
fn leave_active_into(
    machine: &mut Player,
    context: &mut Context,
    constructor: impl FnOnce(Active) -> Player,
) {
    exit_playing_and_active(context);
    // Poisoning is spike (b); a placeholder is enough here.
    let Player::Active(exiting) = core::mem::replace(machine, Player::Idle) else {
        unreachable!("Active is active")
    };
    *machine = constructor(exiting);
    context.note("entry Failed");
}
