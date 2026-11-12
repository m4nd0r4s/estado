use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        #[after(1 + 1)]
        Idle,
        #[unhandled(panik)]
        Busy,
        Active(Active),
    }

    #[state]
    #[history(deep)]
    pub struct Active {
        #[substates]
        sub: Sub,
    }

    #[state]
    pub enum Sub {
        #[initial]
        #[history]
        Playing,
    }
}

fn main() {}
