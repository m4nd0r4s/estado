use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0001, W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
    }

    #[state]
    pub struct Active {
        secret: u8,
        #[substates]
        pub mode: Mode,
    }

    #[state]
    pub enum Mode {
        #[initial]
        Normal,
    }
}

fn main() {
    // The field is private in `Active`; the view must not widen it.
    let mut secret = 7;
    let _ = player::ActiveFields {
        secret: &mut secret,
    };
}
