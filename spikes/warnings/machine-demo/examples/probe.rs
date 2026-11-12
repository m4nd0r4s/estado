//! Manual probe: build with `cargo build -p machine-demo --example probe`
//! and inspect the rustc output. Change `strategy` to compare variants.
use machine_demo::machine;

#[machine]
enum Player {
    Idle,
    #[unreachable_demo]
    Orphan,
    Playing,
    #[unreachable_demo]
    Lost,
}

fn main() {
    let _ = (Player::Idle, Player::Playing, Player::Orphan, Player::Lost);
}
