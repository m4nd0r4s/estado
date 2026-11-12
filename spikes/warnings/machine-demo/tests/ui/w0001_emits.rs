//! W0001 must be reported. `deny(deprecated)` turns the warning into a
//! deterministic error so trybuild can snapshot the rendered diagnostic.
#![deny(deprecated)]

use machine_demo::machine;

#[machine]
enum Player {
    Idle,
    #[unreachable_demo]
    Orphan,
}

fn main() {
    let _ = (Player::Idle, Player::Orphan);
}
