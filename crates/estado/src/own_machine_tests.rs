//! A machine declared inside the `estado` package's own library: the macro
//! finds the package itself (`FoundCrate::Itself`), which cannot name itself
//! with `::estado`, so the generated code must use `::estado_core`. Before
//! the fix this module failed to compile with E0433.

use estado_core::{Machine, Next};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flip {
    Toggle,
}

#[machine(event = Flip, allow(W0001, W0002))]
mod switch {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Switch {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Switch {
        #[on(Off)]
        fn off(_event: &Flip) -> Next<Self> {
            goto!(On)
        }

        #[on(On)]
        fn on(_event: &Flip) -> Next<Self> {
            goto!(Off)
        }
    }
}

#[test]
fn a_machine_inside_the_facade_package_compiles_and_runs() {
    let mut machine = Machine::<switch::Switch>::new(());

    machine.send(Flip::Toggle).unwrap();

    assert!(machine.is_in(switch::Switch::ON));
}
