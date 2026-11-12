use estado_macros::machine;

#[machine(effect = ())]
mod player {
    #[root]
    pub enum Player {
        #[initial]
        Idle,
    }
}

fn main() {}
