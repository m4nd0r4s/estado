//! History gaps found by the M3 reviews: deep history recorded by `reenter!()`
//! and by an external self-transition, a deep history that holds a nested
//! compound state, and a shallow history recorded when a constructor edge
//! exits the compound state.
#![allow(dead_code, missing_docs)]

use estado_core::{Machine, Next};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Enter,
    ToStudy,
    Mess,
    Reset,
    Cycle,
    Restore,
    Resume,
    Leave,
    ShelfEnter,
    ShelfDown,
    ShelfLeave,
    ShelfBack,
}

#[machine(event = Event, allow(W0001, W0002))]
mod memory {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Memory {
        #[initial]
        Away,
        Home(Home),
        Shelf(Shelf),
    }

    #[state]
    #[history(deep, keep)]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Home {
        pub visits: u8,
        #[substates]
        pub room: Room,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Room {
        #[initial]
        Hall,
        Study(Study),
    }

    /// A compound state inside the compound state that keeps a deep history.
    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Study {
        pub books: u8,
        #[substates]
        pub desk: Desk,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Desk {
        #[initial]
        Tidy,
        Messy {
            piles: u8,
        },
    }

    #[state]
    #[history]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Shelf {
        #[substates]
        pub level: Level,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Level {
        #[initial]
        Top,
        Middle,
        Bottom,
    }

    #[handlers]
    impl Memory {
        #[on(Away)]
        fn away(event: &Event) -> Next<Self> {
            match event {
                Event::Enter => goto!(Home { visits: 0 }),
                Event::ShelfEnter => goto!(Shelf),
                Event::Resume => goto!(Home { visits: 9 }, history),
                Event::ShelfBack => goto!(Shelf, history),
                _ => bubble!(),
            }
        }

        #[on(Hall)]
        fn hall(event: &Event) -> Next<Self> {
            match event {
                Event::ToStudy => goto!(Study { books: 2 }),
                Event::Restore => goto!(Home, history),
                _ => bubble!(),
            }
        }

        #[on(Tidy)]
        fn tidy(event: &Event) -> Next<Self> {
            match event {
                Event::Mess => goto!(Messy { piles: 3 }),
                _ => bubble!(),
            }
        }

        #[on(Home)]
        fn home(event: &Event) -> Next<Self> {
            match event {
                Event::Reset => reenter!(),
                Event::Cycle => goto!(Home { visits: 4 }, external),
                Event::Leave => goto!(Away),
                _ => bubble!(),
            }
        }

        #[on(Top)]
        fn top(event: &Event) -> Next<Self> {
            match event {
                Event::ShelfDown => goto!(Middle),
                _ => bubble!(),
            }
        }

        #[on(Middle)]
        fn middle(event: &Event) -> Next<Self> {
            match event {
                Event::ShelfLeave => {
                    goto!(Away, |_old: Exiting![Shelf]| Memory::Away)
                }
                _ => bubble!(),
            }
        }
    }
}

use memory::{Desk, Home, Memory, Room, Study};

fn in_the_messy_study() -> Machine<Memory> {
    let mut machine = Machine::<Memory>::new(());
    for event in [Event::Enter, Event::ToStudy, Event::Mess] {
        machine.send(event).unwrap();
    }
    assert_eq!(machine.path().to_string(), "Home/Study/Messy");
    machine
}

fn the_messy_study() -> Memory {
    Memory::Home(Home {
        visits: 0,
        room: Room::Study(Study {
            books: 2,
            desk: Desk::Messy { piles: 3 },
        }),
    })
}

#[test]
fn reenter_records_the_deep_history_of_the_state_it_exits() {
    let mut machine = in_the_messy_study();

    machine.send(Event::Reset).unwrap();
    assert_eq!(machine.path().to_string(), "Home/Hall");

    // A transition to an ancestor does not exit it: only what `reenter!()`
    // recorded can bring the study back.
    machine.send(Event::Restore).unwrap();
    assert_eq!(*machine.state(), the_messy_study());
}

#[test]
fn an_external_self_transition_records_the_deep_history() {
    let mut machine = in_the_messy_study();

    machine.send(Event::Cycle).unwrap();
    assert_eq!(machine.path().to_string(), "Home/Hall");
    let Memory::Home(Home { visits, .. }) = machine.state() else {
        panic!("Home is active");
    };
    assert_eq!(*visits, 4);

    machine.send(Event::Restore).unwrap();
    let Memory::Home(Home { visits, room }) = machine.state() else {
        panic!("Home is active");
    };
    assert_eq!(*visits, 4);
    assert_eq!(
        *room,
        Room::Study(Study {
            books: 2,
            desk: Desk::Messy { piles: 3 }
        })
    );
}

#[test]
fn deep_history_keeps_a_nested_compound_state_whole() {
    let mut machine = in_the_messy_study();
    machine.send(Event::Leave).unwrap();
    assert_eq!(machine.path().to_string(), "Away");

    // `Home` comes back through its history: the study, the books on it and
    // the pile on the desk, all of it, with the data the caller gave `Home`.
    machine.send(Event::Resume).unwrap();

    assert_eq!(
        *machine.state(),
        Memory::Home(Home {
            visits: 9,
            room: Room::Study(Study {
                books: 2,
                desk: Desk::Messy { piles: 3 }
            })
        })
    );
}

#[test]
fn shallow_history_is_recorded_when_a_constructor_edge_exits_the_compound() {
    let mut machine = Machine::<Memory>::new(());
    for event in [Event::ShelfEnter, Event::ShelfDown, Event::ShelfLeave] {
        machine.send(event).unwrap();
    }
    assert_eq!(machine.path().to_string(), "Away");

    machine.send(Event::ShelfBack).unwrap();

    assert_eq!(machine.path().to_string(), "Shelf/Middle");
}
