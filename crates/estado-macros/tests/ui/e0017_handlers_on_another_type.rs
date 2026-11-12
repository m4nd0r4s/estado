use estado_macros::machine;

pub struct Ev;

pub struct Other;

#[machine(event = Ev, allow(W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
    }

    #[handlers]
    impl Other {
        #[on(Idle)]
        fn on_idle(_event: &Ev) -> Next<Self> {
            stay!()
        }
    }
}

fn main() {}
