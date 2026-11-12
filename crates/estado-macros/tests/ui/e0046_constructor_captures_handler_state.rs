use estado_core::Next;
use estado_macros::machine;

pub enum Ev {
    Crash,
}

#[machine(event = Ev, allow(W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        Failed { code: u8 },
    }

    #[state]
    pub struct Active {
        pub rounds: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    pub enum Mode {
        #[initial]
        Running,
    }

    #[handlers]
    impl Player {
        #[on(Running)]
        fn running(up: ActiveFields<'_>, event: &Ev) -> Next<Self> {
            match event {
                Ev::Crash => goto!(Failed { code: 1 }, |old: Exiting![Active]| Player::Failed {
                    code: *up.rounds + old.rounds,
                }),
            }
        }
    }
}

fn main() {}
