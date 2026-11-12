//! ADR 0013 amendment 17: a machine in a crate whose only route to the core
//! is a renamed `estado-core` dependency runs.

use ec::Machine;
use estado_core_rename_check::{Switch, Toggle};

#[test]
fn a_machine_over_a_renamed_core_crate_runs() {
    let mut machine = Machine::<Switch>::new(());

    machine.send(Toggle::Flip).unwrap();

    assert!(machine.is_in(Switch::ON));
}
