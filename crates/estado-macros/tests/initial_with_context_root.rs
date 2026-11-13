//! `#[initial(with = f)]` where `f` takes the `Cx`, on the root enum and on a
//! chain of initials (errata 15): the machine builds its root value inside
//! `start`, after the first effects of nothing and before the entries.
#![allow(dead_code, missing_docs)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use estado_core::{Cx, Effect, Machine, Next, SendError, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Loaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Boot(u8),
    Entry,
}

/// The number of retries the machine starts with; `u8::MAX` panics.
#[machine(event = Event, effect = Note, ctx = u8, allow(W0002))]
mod boot {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Boot {
        /// Reads the context, emits an effect.
        #[initial(with = loading)]
        Loading {
            retries: u8,
        },
        Ready,
    }

    fn loading<S: Sink<Note>>(cx: &mut Cx<'_, Boot, S>) -> Boot {
        let retries = *cx.ctx();
        assert!(retries != u8::MAX, "the initializer panicked");
        cx.emit(Note::Boot(retries));
        Boot::Loading { retries }
    }

    #[handlers]
    impl Boot {
        #[entry(Loading)]
        fn entry_loading(cx: &mut Cx<Self>) {
            cx.emit(Note::Entry);
        }

        #[on(Loading)]
        fn on_loading(_loading: LoadingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Loaded => goto!(Ready),
            }
        }

        #[on(Ready)]
        fn on_ready(_event: &Event) -> Next<Self> {
            stay!()
        }
    }
}

use boot::Boot;

fn notes(effects: impl IntoIterator<Item = Effect<Note>>) -> Vec<Note> {
    effects
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::User(note) => Some(note),
            _ => None,
        })
        .collect()
}

#[test]
fn the_root_initializer_runs_inside_start_before_the_entries() {
    let mut machine = Machine::<Boot>::new(3);

    let step = machine.start().unwrap();

    assert_eq!(notes(step.effects), [Note::Boot(3), Note::Entry]);
    assert_eq!(*machine.state(), Boot::Loading { retries: 3 });
}

#[test]
fn the_implicit_start_of_a_send_runs_it_too() {
    let mut machine = Machine::<Boot>::new(2);

    let step = machine.send(Event::Loaded).unwrap();

    assert_eq!(notes(step.effects), [Note::Boot(2), Note::Entry]);
    assert_eq!(*machine.state(), Boot::Ready);
}

#[test]
fn a_panic_in_the_root_initializer_poisons_the_machine() {
    let mut machine = Machine::<Boot>::new(u8::MAX);

    let result = catch_unwind(AssertUnwindSafe(|| machine.start()));

    assert!(result.is_err());
    assert!(machine.is_poisoned());
    assert!(matches!(
        machine.send(Event::Loaded),
        Err(SendError::Poisoned)
    ));
}

/// A chain: the root enters `Shell`, which has no data, and its substates
/// start in `Inner`, built by a function that takes the `Cx`.
#[machine(event = Event, effect = Note, allow(W0002))]
mod chain {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Chain {
        #[initial]
        Shell(Shell),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Shell {
        #[substates]
        pub inner: Inner,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Inner {
        #[initial(with = build_inner)]
        Warm(u8),
        Cold,
    }

    /// Only the `Cx`: the fields of `Shell` are not needed.
    fn build_inner<S: Sink<Note>>(cx: &mut Cx<'_, Chain, S>) -> Inner {
        cx.emit(Note::Boot(9));
        Inner::Warm(9)
    }

    #[handlers]
    impl Chain {
        #[on(Warm)]
        fn on_warm(_event: &Event) -> Next<Self> {
            goto!(Cold)
        }

        #[on(Cold)]
        fn on_cold(_event: &Event) -> Next<Self> {
            stay!()
        }
    }
}

#[test]
fn a_chain_of_initials_runs_the_initializer_at_the_end_of_it() {
    use chain::{Chain, Inner, Shell};
    let mut machine = Machine::<Chain>::new(());

    let step = machine.start().unwrap();

    assert_eq!(notes(step.effects), [Note::Boot(9)]);
    assert_eq!(
        *machine.state(),
        Chain::Shell(Shell {
            inner: Inner::Warm(9)
        })
    );
}
