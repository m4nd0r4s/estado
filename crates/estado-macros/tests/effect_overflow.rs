//! A sink that refuses an effect poisons the machine, in the first microstep
//! of a `send`, in an entry of its transition and in a later microstep of the
//! same step alike (ADR 0014); a step with room is unaffected.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Effect, Machine, Next, Overflow, SendError, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Burst,
    Enter,
    Chain,
    Follow,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Beep {
    Short,
}

#[machine(event = Event, effect = Beep, allow(W0002))]
mod beeper {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Beeper {
        #[initial]
        Ready,
        Spent,
        Loud,
        Chained,
    }

    #[handlers]
    impl Beeper {
        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Burst => {
                    cx.emit(Beep::Short);
                    cx.emit(Beep::Short);
                    goto!(Spent)
                }
                Event::Enter => goto!(Loud),
                Event::Chain => {
                    cx.raise(Event::Follow);
                    goto!(Chained)
                }
                _ => bubble!(),
            }
        }

        #[entry(Loud)]
        fn entry_loud(cx: &mut Cx<Self>) {
            cx.emit(Beep::Short);
            cx.emit(Beep::Short);
        }

        #[on(Chained)]
        fn on_chained(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Follow => {
                    cx.emit(Beep::Short);
                    cx.emit(Beep::Short);
                    goto!(Spent)
                }
                _ => bubble!(),
            }
        }
    }
}

use beeper::Beeper;

/// Accepts `room` effects, then overflows.
struct Room {
    room: usize,
    taken: Vec<Effect<Beep>>,
}

impl Room {
    fn of(room: usize) -> Self {
        Self {
            room,
            taken: Vec::new(),
        }
    }
}

impl Sink<Beep> for Room {
    fn push(&mut self, effect: Effect<Beep>) -> Result<(), Overflow> {
        if self.taken.len() == self.room {
            return Err(Overflow);
        }
        self.taken.push(effect);
        Ok(())
    }
}

fn started() -> Machine<Beeper> {
    let mut machine = Machine::<Beeper>::new(());
    machine.start().unwrap();
    machine
}

fn assert_poisoned(machine: &mut Machine<Beeper>) {
    assert!(machine.is_poisoned());
    assert!(matches!(
        machine.send(Event::Burst).unwrap_err(),
        SendError::Poisoned
    ));
    assert!(machine.try_state().is_none());
}

#[test]
fn an_overflow_in_a_handler_poisons_the_machine() {
    let mut machine = started();
    let mut sink = Room::of(1);

    let error = machine.send_into(Event::Burst, &mut sink).unwrap_err();

    assert_eq!(error, SendError::EffectOverflow);
    // The effect that fitted did arrive; the other one is lost.
    assert_eq!(sink.taken, [Effect::User(Beep::Short)]);
    assert_poisoned(&mut machine);
}

#[test]
fn an_overflow_in_an_entry_poisons_the_machine() {
    let mut machine = started();

    let error = machine
        .send_into(Event::Enter, &mut Room::of(1))
        .unwrap_err();

    assert_eq!(error, SendError::EffectOverflow);
    assert_poisoned(&mut machine);
}

#[test]
fn an_overflow_in_a_later_microstep_poisons_the_machine() {
    let mut machine = started();

    let error = machine
        .send_into(Event::Chain, &mut Room::of(1))
        .unwrap_err();

    assert_eq!(error, SendError::EffectOverflow);
    assert_poisoned(&mut machine);
}

#[test]
fn a_sink_with_room_leaves_the_machine_usable() {
    let mut machine = started();

    let report = machine.send_into(Event::Burst, &mut Room::of(2)).unwrap();

    assert!(report.handled);
    assert!(!machine.is_poisoned());
    assert_eq!(*machine.state(), Beeper::Spent);
}

#[test]
fn the_message_says_the_machine_is_poisoned() {
    assert_eq!(
        SendError::<Event>::EffectOverflow.to_string(),
        "the effect buffer is full; the machine is poisoned"
    );
}
