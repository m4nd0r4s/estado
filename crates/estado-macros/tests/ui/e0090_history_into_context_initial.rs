use estado_core::{Cx, Next, Sink};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Stopped,
        Active(Active),
    }

    #[state]
    #[history(deep, keep)]
    pub struct Active {
        #[substates]
        pub mode: Mode,
    }

    #[state]
    pub enum Mode {
        #[initial(with = start)]
        Playing(u8),
        Idle,
    }

    fn start<S: Sink<()>>(cx: &mut Cx<'_, Player, S>) -> Mode {
        let _ = cx;
        Mode::Playing(1)
    }

    #[handlers]
    impl Player {
        #[on(Stopped)]
        fn on_stopped(_event: &Ev) -> Next<Self> {
            goto!(Active, history)
        }

        #[on(Active)]
        fn on_active(_event: &Ev) -> Next<Self> {
            goto!(Stopped)
        }
    }
}

fn main() {}
