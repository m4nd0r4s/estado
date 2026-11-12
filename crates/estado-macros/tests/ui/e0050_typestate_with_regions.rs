use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, typestate)]
mod session {
    #[root]
    pub enum Session {
        #[initial]
        Connected(Connected),
    }

    #[state]
    pub struct Connected {
        #[region]
        link: Link,
        #[region]
        keepalive: KeepAlive,
    }

    #[state]
    pub enum Link {
        #[initial]
        Open,
    }

    #[state]
    pub enum KeepAlive {
        #[initial]
        Idle,
    }
}

fn main() {}
