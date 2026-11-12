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
        #[region]
        output: Output,
    }

    #[state]
    pub enum Output {
        #[initial]
        Speaker,
    }
}

fn main() {}
