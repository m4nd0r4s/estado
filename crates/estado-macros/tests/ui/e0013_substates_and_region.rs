use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Active(Active),
    }

    #[state]
    pub struct Active {
        #[substates]
        sub: Sub,
        #[region]
        extra: Extra,
    }

    #[state]
    pub enum Sub {
        #[initial]
        Playing,
    }

    #[state]
    pub enum Extra {
        #[initial]
        Buffering,
    }
}

fn main() {}
