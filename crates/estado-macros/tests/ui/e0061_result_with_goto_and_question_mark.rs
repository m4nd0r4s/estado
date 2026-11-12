use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Busy,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, _event: &Ev) -> std::result::Result<Next<Self>, String> {
            let _tracks: u8 = "3".parse().map_err(|_| "not a number".to_owned())?;
            Ok(goto!(Busy))
        }
    }
}

fn main() {}
