//! ADR 0013 amendment 15: the E0046 check follows scopes. A name that a
//! `let`, a closure parameter, a match arm or an `if let` binds again is not
//! the handler's parameter, and a field or a named argument that shares the
//! name is not a use. Before the fix every one of these machines was
//! rejected with a false E0046.
#![allow(dead_code, missing_docs)]

use estado_core::{Machine, Next};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    CopiedFirst,
    LetInside,
    NestedParameter,
    MatchArm,
    IfLet,
    FieldAccess,
    NamedArgument,
}

pub struct Report {
    pub up: u8,
}

#[machine(event = Event, allow(W0001, W0002))]
mod player {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        Failed {
            code: u8,
        },
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Active {
        pub rounds: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Running,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn idle(_event: &Event) -> Next<Self> {
            goto!(Active { rounds: 3 })
        }

        #[on(Running)]
        fn running(up: ActiveFields<'_>, event: &Event) -> Next<Self> {
            match event {
                // The fix the E0046 page recommends.
                Event::CopiedFirst => {
                    let up = *up.rounds;
                    goto!(Failed { code: 0 }, move |old: Exiting![Active]| {
                        Player::Failed {
                            code: up + old.rounds,
                        }
                    })
                }
                Event::LetInside => goto!(Failed { code: 0 }, |old: Exiting![Active]| {
                    let up = old.rounds;
                    Player::Failed { code: up + 1 }
                }),
                Event::NestedParameter => goto!(Failed { code: 0 }, |old: Exiting![Active]| {
                    let bump = |up: u8| up + 2;
                    Player::Failed {
                        code: bump(old.rounds),
                    }
                }),
                Event::MatchArm => goto!(Failed { code: 0 }, |old: Exiting![Active]| {
                    match Some(old.rounds) {
                        Some(up) => Player::Failed { code: up + 3 },
                        None => Player::Failed { code: 0 },
                    }
                }),
                Event::IfLet => goto!(Failed { code: 0 }, |old: Exiting![Active]| {
                    if let Some(up) = Some(old.rounds) {
                        Player::Failed { code: up + 4 }
                    } else {
                        Player::Failed { code: 0 }
                    }
                }),
                Event::FieldAccess => goto!(Failed { code: 0 }, |old: Exiting![Active]| {
                    let report = Report { up: old.rounds };
                    Player::Failed {
                        code: format!("{}", report.up).parse().unwrap(),
                    }
                }),
                Event::NamedArgument => goto!(Failed { code: 0 }, |old: Exiting![Active]| {
                    Player::Failed {
                        code: format!("{up}", up = old.rounds).parse().unwrap(),
                    }
                }),
            }
        }
    }
}

use player::Player;

fn code_after(event: Event) -> u8 {
    let mut machine = Machine::<Player>::new(());
    machine.send(Event::CopiedFirst).unwrap();
    machine.send(event).unwrap();
    match machine.state() {
        Player::Failed { code } => *code,
        other => panic!("not failed: {other:?}"),
    }
}

#[test]
fn a_local_that_shadows_the_parameter_is_not_the_parameter() {
    assert_eq!(code_after(Event::CopiedFirst), 6);
}

#[test]
fn a_let_inside_the_constructor_shadows_the_parameter() {
    assert_eq!(code_after(Event::LetInside), 4);
}

#[test]
fn a_nested_closure_parameter_shadows_the_parameter() {
    assert_eq!(code_after(Event::NestedParameter), 5);
}

#[test]
fn a_match_arm_binding_shadows_the_parameter() {
    assert_eq!(code_after(Event::MatchArm), 6);
}

#[test]
fn an_if_let_binding_shadows_the_parameter() {
    assert_eq!(code_after(Event::IfLet), 7);
}

#[test]
fn a_field_with_the_name_of_the_parameter_is_not_a_use() {
    assert_eq!(code_after(Event::FieldAccess), 3);
}

#[test]
fn a_named_format_argument_with_the_name_of_the_parameter_is_not_a_use() {
    assert_eq!(code_after(Event::NamedArgument), 3);
}
