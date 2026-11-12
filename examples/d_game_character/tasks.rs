//! Timers of the states (spec 5.3 steps 2, 8 and 9, spec 7.1): one pair of
//! functions per region with `#[after]` states.
//!
//! A key is the state, its epoch and the index of the timer inside the state.
//! `start_*` moves the epoch on and schedules; `cancel_*` uses the epoch of
//! the instance that is leaving. This machine has no `#[invoke]`.

use estado_core::{ChildPath, Effect, Sink, StateId, TimerKey};

use super::actions::CharacterCx;
use super::model::{Character, Command, FIRE_COOLDOWN, JUMP_TIME, Movement, RELOAD_TIME, Weapon};
use super::runtime::next_epoch;

const fn timer(state: StateId, epoch: u32) -> TimerKey {
    TimerKey {
        state,
        epoch,
        index: 0,
        child: ChildPath::ROOT,
    }
}

/// `Jumping`'s `#[after]`.
pub(super) const fn jump_timer(epoch: u32) -> TimerKey {
    timer(Character::JUMPING, epoch)
}

/// `Firing`'s `#[after]`.
pub(super) const fn firing_timer(epoch: u32) -> TimerKey {
    timer(Character::FIRING, epoch)
}

/// `Reloading`'s `#[after]`.
pub(super) const fn reload_timer(epoch: u32) -> TimerKey {
    timer(Character::RELOADING, epoch)
}

/// Tasks of the entered leaf of the `movement` region.
pub(super) fn start_movement<S: Sink<Command>>(movement: Movement, cx: &mut CharacterCx<'_, S>) {
    if let Movement::Jumping = movement {
        let epoch = next_epoch(&mut cx.runtime_mut().jumping_epoch);
        cx.effect(Effect::StartTimer {
            key: jump_timer(epoch),
            after: JUMP_TIME,
        });
    }
}

/// Tasks of the exiting leaf of the `movement` region.
pub(super) fn cancel_movement<S: Sink<Command>>(movement: Movement, cx: &mut CharacterCx<'_, S>) {
    if let Movement::Jumping = movement {
        let epoch = cx.runtime_mut().jumping_epoch;
        cx.effect(Effect::CancelTimer(jump_timer(epoch)));
    }
}

/// Tasks of the entered leaf of the `weapon` region.
pub(super) fn start_weapon<S: Sink<Command>>(weapon: Weapon, cx: &mut CharacterCx<'_, S>) {
    match weapon {
        Weapon::Firing => {
            let epoch = next_epoch(&mut cx.runtime_mut().firing_epoch);
            cx.effect(Effect::StartTimer {
                key: firing_timer(epoch),
                after: FIRE_COOLDOWN,
            });
        }
        Weapon::Reloading => {
            let epoch = next_epoch(&mut cx.runtime_mut().reloading_epoch);
            cx.effect(Effect::StartTimer {
                key: reload_timer(epoch),
                after: RELOAD_TIME,
            });
        }
        Weapon::Ready | Weapon::Empty => {}
    }
}

/// Tasks of the exiting leaf of the `weapon` region.
pub(super) fn cancel_weapon<S: Sink<Command>>(weapon: Weapon, cx: &mut CharacterCx<'_, S>) {
    match weapon {
        Weapon::Firing => {
            let epoch = cx.runtime_mut().firing_epoch;
            cx.effect(Effect::CancelTimer(firing_timer(epoch)));
        }
        Weapon::Reloading => {
            let epoch = cx.runtime_mut().reloading_epoch;
            cx.effect(Effect::CancelTimer(reload_timer(epoch)));
        }
        Weapon::Ready | Weapon::Empty => {}
    }
}

/// Schedules the tasks of everything that just entered, parents first and
/// regions in declaration order (spec 5.3 step 9).
pub(super) fn start_root<S: Sink<Command>>(root: &Character, cx: &mut CharacterCx<'_, S>) {
    match root {
        Character::Alive(alive) => {
            start_movement(alive.movement, cx);
            start_weapon(alive.weapon, cx);
        }
        Character::Dead => {}
    }
}

/// Cancels the tasks of everything that just exited, deepest first and
/// regions in reverse declaration order (spec 5.3 step 2).
pub(super) fn cancel_root<S: Sink<Command>>(root: &Character, cx: &mut CharacterCx<'_, S>) {
    match root {
        Character::Alive(alive) => {
            cancel_weapon(alive.weapon, cx);
            cancel_movement(alive.movement, cx);
        }
        Character::Dead => {}
    }
}
