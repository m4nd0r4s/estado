//! Shared helpers of the scenario I tests.

use std::time::Duration;

use estado_core::Machine;
use estado_examples::i_media_player::{Active, AudioBuffer, Command, Event, Player, TrackId};

pub const SECOND: Duration = Duration::from_secs(1);

/// Compiles only when `AudioBuffer` does not implement `Clone`: with `Clone`
/// the two blanket impls overlap and the associated function is ambiguous.
const _: () = {
    trait AmbiguousIfClone<Marker> {
        fn probe() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}
    let _ = <AudioBuffer as AmbiguousIfClone<_>>::probe;
};

pub fn idle() -> Machine<Player> {
    let mut machine = Machine::<Player>::new(Vec::new());
    machine.start().unwrap();
    machine.ctx_mut().clear();
    machine
}

/// In `Active/Playing` on track 7, with an empty log.
pub fn playing() -> Machine<Player> {
    let mut machine = idle();
    machine.send(Event::Play(TrackId(7))).unwrap();
    machine.ctx_mut().clear();
    machine
}

/// In `Active/Paused` at one second, with an empty log.
pub fn paused() -> Machine<Player> {
    let mut machine = playing();
    machine.send(Event::Tick(SECOND)).unwrap();
    machine.send(Event::Pause).unwrap();
    machine.ctx_mut().clear();
    machine
}

/// Sends `event`; returns the log and the user commands it emitted.
pub fn send(machine: &mut Machine<Player>, event: Event) -> (Vec<String>, Vec<Command>) {
    let step = machine.send(event).unwrap();
    let commands = step.effects.commands().cloned().collect();
    (std::mem::take(machine.ctx_mut()), commands)
}

pub fn active_of(machine: &Machine<Player>) -> &Active {
    match machine.state() {
        Player::Active(active) => active,
        other => panic!("expected Active, got {other:?}"),
    }
}

pub fn failed_of(machine: &Machine<Player>) -> (&str, &AudioBuffer) {
    match machine.state() {
        Player::Failed { reason, buffer } => (reason, buffer),
        other => panic!("expected Failed, got {other:?}"),
    }
}
