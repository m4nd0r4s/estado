//! ADR 0013 items 6 and 9: a machine whose every type is private, under
//! `#![deny(warnings)]`. The generated code must not leak the user's private
//! types (`private_interfaces`, `private_bounds`), views must not widen the
//! visibility of the fields they borrow, and the helpers generated on the
//! user's types must not collide with the user's own methods.
//!
//! The machine uses most of what the macro generates: a deep history kept by a
//! constructor edge, a struct-like variant with a timer (its `*Ref` view),
//! a parallel state with a completion, `#[always]`, `cx.raise` and `defer!()`.
#![deny(warnings)]
#![allow(missing_docs)]

use std::time::Duration;

use estado_core::{Cx, Effect, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Launch,
    Dock,
    Crash,
    Tick,
    Open,
    Light,
    Reset,
    Work,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Note(&'static str),
}

fn note<S: Sink<Command>>(cx: &mut Cx<'_, harbor::Harbor, S>, line: &'static str) {
    cx.emit(Command::Note(line));
}

#[machine(event = Event, effect = Command, raise_cap = 4, defer_cap = 4)]
mod harbor {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum Harbor {
        #[initial]
        Idle,
        Active(Active),
        Fault {
            code: u8,
        },
        Gate(Gate),
        #[terminal]
        Retired,
    }

    #[state]
    #[history(deep, keep)]
    #[derive(Debug, PartialEq, Eq)]
    pub(super) struct Active {
        pub(super) name: String,
        #[substates]
        pub(super) mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum Mode {
        #[initial]
        Cruising,
        #[after(Duration::from_millis(5) => Event::Tick)]
        Docking { progress: u8 },
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub(super) struct Gate {
        pub(super) opened: bool,
        #[region]
        pub(super) door: Door,
        #[region]
        pub(super) light: Light,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum Door {
        #[initial]
        Shut,
        #[terminal]
        Ajar,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum Light {
        #[initial]
        Dark,
        #[terminal]
        Bright,
    }

    impl Active {
        /// A method of the user's that the generated helpers must not shadow.
        pub(super) fn split(&self) -> usize {
            self.name.len()
        }
    }

    #[handlers]
    impl Harbor {
        #[entry(Active)]
        fn enter_active(active: ActiveFields<'_>, cx: &mut Cx<Self>) {
            let _ = active.name.len();
            note(cx, "entry Active");
        }

        #[on(Idle)]
        fn idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Launch => goto!(Active {
                    name: "ship".to_owned()
                }),
                Event::Open => goto!(Gate { opened: false }),
                Event::Work => {
                    cx.raise(Event::Reset);
                    defer!()
                }
                _ => bubble!(),
            }
        }

        #[on(Cruising)]
        fn cruising(event: &Event) -> Next<Self> {
            match event {
                Event::Dock => goto!(Docking { progress: 0 }),
                _ => bubble!(),
            }
        }

        #[on(Docking)]
        fn docking(view: DockingMut<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Tick => {
                    *view.progress += 1;
                    stay!()
                }
                _ => bubble!(),
            }
        }

        #[on(Active)]
        fn active(active: ActiveFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Crash => {
                    let code = active.name.len() as u8;
                    goto!(Fault { code }, move |old: Exiting![Active]| {
                        Harbor::Fault {
                            code: code + old.name.len() as u8,
                        }
                    })
                }
                _ => bubble!(),
            }
        }

        #[on(Fault)]
        fn fault(event: &Event) -> Next<Self> {
            match event {
                Event::Reset => goto!(Idle),
                _ => bubble!(),
            }
        }

        #[on(Shut)]
        fn shut(event: &Event) -> Next<Self> {
            match event {
                Event::Open => goto!(Ajar),
                _ => bubble!(),
            }
        }

        #[on(Dark)]
        fn dark(event: &Event) -> Next<Self> {
            match event {
                Event::Light => goto!(Bright),
                _ => bubble!(),
            }
        }

        #[on_done(Gate)]
        fn gate_done(gate: GateFields<'_>, cx: &mut Cx<Self>) -> Next<Self> {
            *gate.opened = true;
            note(cx, "gate done");
            goto!(Retired)
        }
    }
}

use harbor::Harbor;

fn run(machine: &mut Machine<Harbor>, event: Event) -> Vec<Command> {
    let mut sink: Vec<Effect<Command>> = Vec::new();
    machine
        .send_with(event, |effect| sink.push(effect))
        .unwrap();
    sink.into_iter()
        .filter_map(|effect| match effect {
            Effect::User(command) => Some(command),
            _ => None,
        })
        .collect()
}

#[test]
fn a_machine_of_private_types_compiles_without_warnings_and_runs() {
    let mut machine = Machine::<Harbor>::new(());
    run(&mut machine, Event::Work);
    run(&mut machine, Event::Launch);
    assert_eq!(machine.path().to_string(), "Active/Cruising");
    run(&mut machine, Event::Dock);
    assert_eq!(machine.path().to_string(), "Active/Docking");
    run(&mut machine, Event::Crash);
    assert_eq!(machine.path().to_string(), "Fault");
    run(&mut machine, Event::Reset);
    run(&mut machine, Event::Open);
    run(&mut machine, Event::Open);
    let commands = run(&mut machine, Event::Light);
    assert_eq!(commands, [Command::Note("gate done")]);
    assert!(machine.is_done());
}

#[test]
fn a_user_method_named_split_does_not_collide_with_the_generated_helpers() {
    let active = harbor::Active {
        name: "abc".to_owned(),
        mode: harbor::Mode::Cruising,
    };
    assert_eq!(active.split(), 3);
}
