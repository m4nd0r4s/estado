//! `#[machine(allow(W0001))]` suppresses the warning: the crate compiles even
//! with `deny(deprecated)`.
#![deny(deprecated)]

use machine_demo::machine;

#[machine(allow(W0001))]
enum Player {
    Idle,
    #[unreachable_demo]
    Orphan,
}

fn main() {
    let _ = (Player::Idle, Player::Orphan);
}
