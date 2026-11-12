use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod job {
    use super::*;

    #[root]
    pub enum Job {
        #[initial]
        Running(Running),
    }

    #[state]
    pub struct Running {
        #[substates]
        pub step: Step,
    }

    #[state]
    pub enum Step {
        #[initial]
        Working,
        #[terminal]
        Complete,
    }

    #[handlers]
    impl Job {
        #[on_done(Running)]
        fn running_done(_cx: &mut Cx<Self>) -> Next<Self> {
            bubble!()
        }
    }
}

fn main() {}
