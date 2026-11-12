use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod link {
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
        Sending(Nested),
    }

    #[state]
    pub struct Nested {
        #[region]
        left: Left,
        #[region]
        right: Right,
    }

    #[state]
    pub enum Left {
        #[initial]
        Lone,
    }

    #[state]
    pub enum Right {
        #[initial]
        Solo,
    }

    #[state]
    pub enum Receiver {
        #[initial]
        Listening,
    }
}

fn main() {}
