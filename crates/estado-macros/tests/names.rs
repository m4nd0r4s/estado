//! ADR 0013 items 7, 10 and 12 (names): raw identifiers as state names, edge
//! variants of states called `Foo` and `Foo2` in one handler, and generated
//! identifiers that must not depend on how the user spelled a name.
//!
//! Before the fix a state called `r#loop` made the macro panic (`Tor#loop`,
//! `r#loopMut`) and `goto!(Foo)` twice next to `goto!(Foo2)` generated two
//! edge variants named `ToFoo2`.
#![allow(dead_code, missing_docs, non_camel_case_types)]

use std::time::Duration;

use estado_core::{Machine, Next};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ToFoo2,
    ToFoo,
    ToFooAgain,
    EnterLoop,
    Tick,
    Leave,
}

#[machine(event = Event, allow(W0002))]
mod names {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Names {
        #[initial]
        Foo,
        Foo2,
        #[after(Duration::from_millis(1) => Event::Tick)]
        r#loop {
            n: u8,
        },
        r#type(u8),
    }

    #[handlers]
    impl Names {
        #[on(Foo)]
        fn foo(event: &Event) -> Next<Self> {
            match event {
                Event::ToFoo => goto!(Foo),
                Event::ToFooAgain => goto!(Foo),
                Event::ToFoo2 => goto!(Foo2),
                Event::EnterLoop => goto!(r#loop { n: 1 }),
                _ => bubble!(),
            }
        }

        #[on(r#loop)]
        fn on_loop(view: loopMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Tick => {
                    *view.n += 1;
                    stay!()
                }
                Event::Leave => goto!(r#type(7), |old: Exiting![r#loop]| Names::r#type(old.n)),
                _ => bubble!(),
            }
        }

        #[on(r#type)]
        fn on_type(_value: &mut u8, event: &Event) -> Next<Self> {
            match event {
                Event::ToFoo => goto!(Foo),
                _ => bubble!(),
            }
        }
    }
}

use names::Names;

#[test]
fn edge_variants_of_foo_and_foo2_do_not_collide() {
    let mut machine = Machine::<Names>::new(());
    machine.send(Event::ToFoo).unwrap();
    machine.send(Event::ToFooAgain).unwrap();
    machine.send(Event::ToFoo2).unwrap();
    assert_eq!(*machine.state(), Names::Foo2);
}

#[test]
fn raw_identifier_states_expand_and_run() {
    let mut machine = Machine::<Names>::new(());
    machine.send(Event::EnterLoop).unwrap();
    assert_eq!(*machine.state(), Names::r#loop { n: 1 });
    machine.send(Event::Tick).unwrap();
    assert_eq!(*machine.state(), Names::r#loop { n: 2 });

    machine.send(Event::Leave).unwrap();
    assert_eq!(*machine.state(), Names::r#type(2));
    machine.send(Event::ToFoo).unwrap();
    assert_eq!(*machine.state(), Names::Foo);
    assert_eq!(machine.path().to_string(), "Foo");
}

#[test]
fn the_graph_names_raw_states_without_the_prefix() {
    use estado_core::Spec;
    let names: Vec<&str> = Names::GRAPH.states.iter().map(|state| state.name).collect();
    assert_eq!(names, ["Foo", "Foo2", "loop", "type"]);
}
