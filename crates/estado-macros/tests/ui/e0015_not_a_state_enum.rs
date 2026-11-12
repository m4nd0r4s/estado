use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Active(Active),
        Other(Other),
    }

    #[state]
    pub struct Active {
        #[substates]
        sub: ActiveSub,
    }

    // Missing `#[state]`.
    pub enum ActiveSub {
        Playing,
    }

    #[state]
    pub struct Other {
        #[substates]
        sub: u32,
    }
}

fn main() {}
