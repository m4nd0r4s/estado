//! Feature detection sees the path forms of `raise` and `in_state`, not only
//! the method calls: `Cx::raise(cx, ..)`, `Cx::<..>::raise(..)`,
//! `::estado_core::Cx::in_state(..)`. Before, a machine whose only use was
//! written that way had no queues (the raise overflowed and poisoned the
//! machine) and `in_state` was false for ever.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Go,
    Note,
}

#[machine(event = Event, ctx = Vec<&'static str>, allow(W0001, W0002))]
mod plain_path {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum PlainPath {
        #[initial]
        Start,
        Done,
    }

    #[handlers]
    impl PlainPath {
        #[on(Start)]
        fn start(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Go => {
                    Cx::raise(cx, Event::Note);
                    goto!(Done)
                }
                _ => bubble!(),
            }
        }

        #[on(Done)]
        fn done(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            if let Event::Note = event {
                cx.ctx_mut().push("note");
            }
            stay!()
        }
    }
}

#[machine(event = Event, ctx = Vec<&'static str>, allow(W0001, W0002))]
mod turbofish_path {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum TurbofishPath {
        #[initial]
        Start,
        Done,
    }

    #[handlers]
    impl TurbofishPath {
        #[on(Start)]
        fn start(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Go => {
                    ::estado_core::Cx::<'_, Self, _>::raise(cx, Event::Note);
                    goto!(Done)
                }
                _ => bubble!(),
            }
        }

        #[on(Done)]
        fn done(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            if let Event::Note = event {
                cx.ctx_mut().push("note");
            }
            stay!()
        }
    }
}

/// The call is in a helper inside the module, not in a handler.
#[machine(event = Event, ctx = Vec<&'static str>, allow(W0001, W0002))]
mod helper_path {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum HelperPath {
        #[initial]
        Start,
        Done,
    }

    fn note_where_we_are<S: ::estado_core::Sink<()>>(cx: &mut Cx<'_, HelperPath, S>) {
        let on_start = Cx::in_state(cx, HelperPath::START);
        cx.ctx_mut()
            .push(if on_start { "start" } else { "elsewhere" });
    }

    #[handlers]
    impl HelperPath {
        #[on(Start)]
        fn start(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Go => {
                    note_where_we_are(cx);
                    goto!(Done)
                }
                _ => bubble!(),
            }
        }
    }
}

#[test]
fn a_raise_written_as_a_path_call_turns_the_queues_on() {
    let mut machine = Machine::<plain_path::PlainPath>::new(Vec::new());

    machine.send(Event::Go).unwrap();

    assert!(!machine.is_poisoned());
    assert_eq!(machine.ctx().as_slice(), ["note"]);
}

#[test]
fn a_raise_through_a_path_with_generics_turns_the_queues_on() {
    let mut machine = Machine::<turbofish_path::TurbofishPath>::new(Vec::new());

    machine.send(Event::Go).unwrap();

    assert!(!machine.is_poisoned());
    assert_eq!(machine.ctx().as_slice(), ["note"]);
}

#[test]
fn an_in_state_written_as_a_path_call_in_a_helper_turns_tracking_on() {
    let mut machine = Machine::<helper_path::HelperPath>::new(Vec::new());

    machine.send(Event::Go).unwrap();

    assert_eq!(machine.ctx().as_slice(), ["start"]);
}
