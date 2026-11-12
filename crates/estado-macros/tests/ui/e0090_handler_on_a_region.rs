use estado_core::{Cx, Next};
use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod link {
    use super::*;

    #[root]
    pub enum Link {
        #[initial]
        Up(Up),
    }

    #[state]
    pub struct Up {
        #[region]
        sender: Sender,
        #[region]
        receiver: Receiver,
    }

    #[state]
    pub enum Sender {
        #[initial]
        Sending,
    }

    #[state]
    pub enum Receiver {
        #[initial]
        Listening,
    }

    #[handlers]
    impl Link {
        #[on(sender)]
        fn on_sender(_cx: &mut Cx<Self>, _event: &Ev) -> Next<Self> {
            bubble!()
        }
    }
}

fn main() {}
