//! The shape of the edges of a handler (BM-01, ADR 0017): a plain `goto!`
//! carries the data of its target, not the built value; several `goto!`s of
//! one handler that do the same share one `apply` arm; and a zero-parameter
//! `#[initial(with = f)]` still runs where the `goto!` is written, before the
//! exits (ADR 0016).
#![allow(dead_code, missing_docs)]

use std::sync::Mutex;

use estado_core::{Cx, Machine, Next};
use estado_macros::machine;

/// Order in which the initializer and the exit handler ran.
static LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

fn record(what: &'static str) {
    LOG.lock().unwrap().push(what);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Odd,
    Big,
    Zero,
}

#[machine(event = u8, ctx = (), allow(W0001, W0002))]
mod parser {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Parser {
        #[initial]
        Start,
        Reading,
        Settled(Settled),
        /// Makes the value of the root large, so that an edge that carried it
        /// would be large too.
        Wide([u64; 4]),
        #[terminal]
        Failed(Reason),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Settled {
        pub seen: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial(with = new_quiet)]
        Quiet,
        Loud,
    }

    fn new_quiet() -> Mode {
        record("initial");
        Mode::Quiet
    }

    #[handlers]
    impl Parser {
        #[exit(Start)]
        fn exit_start(_cx: &mut Cx<Self>) {
            record("exit");
        }

        #[on(Start)]
        fn on_start(event: &u8) -> Next<Self> {
            match *event {
                100 => goto!(Reading),
                _ => goto!(Settled { seen: *event }),
            }
        }

        #[on(Reading)]
        fn on_reading(event: &u8) -> Next<Self> {
            match *event {
                0 => goto!(Failed(Reason::Zero)),
                1..=9 if *event % 2 == 1 => goto!(Failed(Reason::Odd)),
                _ => goto!(Failed(Reason::Big)),
            }
        }
    }
}

#[test]
fn a_plain_goto_carries_the_data_of_its_target_not_the_built_value() {
    // The largest variant carries one `Reason` or one `u8`.
    let edge = core::mem::size_of::<parser::__estado::ReadingEdges>();
    assert!(edge <= 2, "the edge of `Reading` is {edge} bytes");
    assert!(core::mem::size_of::<parser::Parser>() >= 32);
}

/// One test, because the log is shared: the exits of other tests would land
/// in it.
#[test]
fn gotos_that_do_the_same_reach_their_own_data_and_an_initializer_runs_first() {
    for (event, reason) in [(0, Reason::Zero), (3, Reason::Odd), (250, Reason::Big)] {
        let mut machine = Machine::<parser::Parser>::new(());
        machine.send(100).unwrap();
        machine.send(event).unwrap();
        assert_eq!(machine.state(), &parser::Parser::Failed(reason));
    }

    LOG.lock().unwrap().clear();
    let mut machine = Machine::<parser::Parser>::new(());
    machine.send(20).unwrap();
    assert_eq!(*LOG.lock().unwrap(), ["initial", "exit"]);
    assert!(matches!(
        machine.state(),
        parser::Parser::Settled(parser::Settled { seen: 20, .. })
    ));
}
