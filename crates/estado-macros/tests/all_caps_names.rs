//! States named in all capitals (`A`, `B`, `IO`) work: their id constants
//! carry the suffix `_ID`, so they never clash with the variant of the same
//! name (before, `Bus::IO` was both and the machine was rejected with E0012).
#![allow(dead_code, missing_docs)]

use estado_core::{Machine, Next, Spec};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ev {
    Next,
}

#[machine(event = Ev, allow(W0002))]
mod bus {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Bus {
        #[initial]
        A,
        B,
        IO,
    }

    #[handlers]
    impl Bus {
        #[on(A)]
        fn on_a(event: &Ev) -> Next<Self> {
            match event {
                Ev::Next => goto!(B),
            }
        }

        #[on(B)]
        fn on_b(event: &Ev) -> Next<Self> {
            match event {
                Ev::Next => goto!(IO),
            }
        }

        #[on(IO)]
        fn on_io(event: &Ev) -> Next<Self> {
            match event {
                Ev::Next => goto!(A),
            }
        }
    }
}

use bus::Bus;

#[test]
fn single_letter_and_all_caps_states_get_suffixed_id_constants() {
    assert_eq!(Bus::A_ID.index(), 0);
    assert_eq!(Bus::B_ID.index(), 1);
    assert_eq!(Bus::IO_ID.index(), 2);
    assert_eq!(<Bus as Spec>::GRAPH.state(Bus::IO_ID).name, "IO");
}

#[test]
fn a_machine_with_all_caps_states_runs() {
    let mut machine = Machine::<Bus>::new(());
    machine.start().unwrap();
    assert!(machine.is_in(Bus::A_ID));
    machine.send(Ev::Next).unwrap();
    assert!(machine.is_in(Bus::B_ID));
    machine.send(Ev::Next).unwrap();
    assert!(machine.is_in(Bus::IO_ID));
    machine.send(Ev::Next).unwrap();
    assert!(machine.is_in(Bus::A_ID));
}
