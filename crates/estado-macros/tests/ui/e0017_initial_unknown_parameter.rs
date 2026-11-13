use estado_core::Next;
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
    pub struct Active {
        pub volume: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    pub enum Mode {
        #[initial(with = start)]
        Playing(u8),
        Idle,
    }

    // `volume` is not something an initializer can receive: it takes the
    // `Cx` and `ActiveFields`.
    fn start(volume: u8) -> Mode {
        Mode::Playing(volume)
    }

    #[handlers]
    impl Player {
        #[on(Stopped)]
        fn on_stopped(_event: &Ev) -> Next<Self> {
            goto!(Active { volume: 1 })
        }

        #[on(Active)]
        fn on_active(_event: &Ev) -> Next<Self> {
            goto!(Stopped)
        }
    }
}

fn main() {}
