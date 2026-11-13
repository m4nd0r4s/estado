//! `#[initial(with = f)]` where `f` takes the `Cx` and the fields of the
//! parent state (errata 15): it runs when the state is entered, after the
//! exits of the transition and before the entries, and may emit effects and
//! read the global context.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Effect, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play,
    Louder,
    Restart,
    Back,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    ExitStopped,
    Loaded(u8),
    EntryPlaying,
}

#[machine(event = Event, effect = Note, ctx = u32, allow(W0001, W0002))]
mod player {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Stopped,
        Active(Active),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub volume: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial(with = start_playing)]
        Playing(Track),
        Idle,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct Track(pub u32);

    /// The spec form: the `Cx`, then the fields of the parent `Active`.
    fn start_playing<S: Sink<Note>>(cx: &mut Cx<'_, Player, S>, fields: ActiveFields<'_>) -> Mode {
        cx.emit(Note::Loaded(*fields.volume));
        *cx.ctx_mut() += 1;
        Mode::Playing(Track(u32::from(*fields.volume) * 100))
    }

    #[handlers]
    impl Player {
        #[exit(Stopped)]
        fn exit_stopped(cx: &mut Cx<Self>) {
            cx.emit(Note::ExitStopped);
        }

        #[entry(Playing)]
        fn entry_playing(cx: &mut Cx<Self>) {
            cx.emit(Note::EntryPlaying);
        }

        #[on(Stopped)]
        fn on_stopped(event: &Event) -> Next<Self> {
            match event {
                Event::Play => goto!(Active { volume: 5 }),
                _ => unhandled!(),
            }
        }

        #[on(Active)]
        fn on_active(active: ActiveFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Louder => {
                    *active.volume += 1;
                    stay!()
                }
                Event::Restart => reenter!(),
                Event::Stop => goto!(Stopped),
                _ => unhandled!(),
            }
        }

        #[on(Playing)]
        fn on_playing(_track: &mut Track, event: &Event) -> Next<Self> {
            match event {
                // A local transition to the ancestor: its substates restart
                // from the initial, with the fields `Active` keeps.
                Event::Back => goto!(Active),
                _ => bubble!(),
            }
        }
    }
}

use player::{Active, Mode, Player, Track};

fn notes(effects: impl IntoIterator<Item = Effect<Note>>) -> Vec<Note> {
    effects
        .into_iter()
        .map(|effect| match effect {
            Effect::User(note) => note,
            other => panic!("unexpected effect {other:?}"),
        })
        .collect()
}

fn playing(volume: u8, track: u32) -> Player {
    Player::Active(Active {
        volume,
        mode: Mode::Playing(Track(track)),
    })
}

#[test]
fn a_goto_runs_the_initializer_with_the_data_it_supplied_after_the_exits() {
    let mut machine = Machine::<Player>::new(0);
    machine.start().unwrap();

    let step = machine.send(Event::Play).unwrap();

    // Exit of the old state, the initializer, then the entries.
    assert_eq!(
        notes(step.effects),
        [Note::ExitStopped, Note::Loaded(5), Note::EntryPlaying]
    );
    assert_eq!(*machine.state(), playing(5, 500));
    // The initializer read and wrote the global context.
    assert_eq!(*machine.ctx(), 1);
}

#[test]
fn reenter_runs_the_initializer_again_with_the_fields_the_state_kept() {
    let mut machine = Machine::<Player>::new(0);
    machine.send(Event::Play).unwrap();
    machine.send(Event::Louder).unwrap();

    let step = machine.send(Event::Restart).unwrap();

    assert_eq!(notes(step.effects), [Note::Loaded(6), Note::EntryPlaying]);
    assert_eq!(*machine.state(), playing(6, 600));
    assert_eq!(*machine.ctx(), 2);
}

#[test]
fn a_local_goto_to_the_ancestor_gives_the_initializer_its_live_fields() {
    let mut machine = Machine::<Player>::new(0);
    machine.send(Event::Play).unwrap();
    machine.send(Event::Louder).unwrap();
    machine.send(Event::Louder).unwrap();

    let step = machine.send(Event::Back).unwrap();

    assert_eq!(notes(step.effects), [Note::Loaded(7), Note::EntryPlaying]);
    assert_eq!(*machine.state(), playing(7, 700));
}

#[test]
fn a_transition_without_an_initial_in_its_path_runs_no_initializer() {
    let mut machine = Machine::<Player>::new(0);
    machine.send(Event::Play).unwrap();

    let step = machine.send(Event::Stop).unwrap();

    assert!(notes(step.effects).is_empty());
    assert_eq!(*machine.state(), Player::Stopped);
    assert_eq!(*machine.ctx(), 1);
}
