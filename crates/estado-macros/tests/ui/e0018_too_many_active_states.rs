use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev, allow(W0002))]
mod wide {
    #[root]
    pub enum Root {
        #[initial]
        Wide(Wide),
    }

    #[state]
    pub struct Wide {
        #[region]
        region_0: Part0,
        #[region]
        region_1: Part1,
        #[region]
        region_2: Part2,
        #[region]
        region_3: Part3,
        #[region]
        region_4: Part4,
        #[region]
        region_5: Part5,
        #[region]
        region_6: Part6,
        #[region]
        region_7: Part7,
        #[region]
        region_8: Part8,
        #[region]
        region_9: Part9,
        #[region]
        region_10: Part10,
        #[region]
        region_11: Part11,
        #[region]
        region_12: Part12,
        #[region]
        region_13: Part13,
        #[region]
        region_14: Part14,
        #[region]
        region_15: Part15,
    }

    #[state]
    pub enum Part0 {
        #[initial]
        Leaf0,
    }

    #[state]
    pub enum Part1 {
        #[initial]
        Leaf1,
    }

    #[state]
    pub enum Part2 {
        #[initial]
        Leaf2,
    }

    #[state]
    pub enum Part3 {
        #[initial]
        Leaf3,
    }

    #[state]
    pub enum Part4 {
        #[initial]
        Leaf4,
    }

    #[state]
    pub enum Part5 {
        #[initial]
        Leaf5,
    }

    #[state]
    pub enum Part6 {
        #[initial]
        Leaf6,
    }

    #[state]
    pub enum Part7 {
        #[initial]
        Leaf7,
    }

    #[state]
    pub enum Part8 {
        #[initial]
        Leaf8,
    }

    #[state]
    pub enum Part9 {
        #[initial]
        Leaf9,
    }

    #[state]
    pub enum Part10 {
        #[initial]
        Leaf10,
    }

    #[state]
    pub enum Part11 {
        #[initial]
        Leaf11,
    }

    #[state]
    pub enum Part12 {
        #[initial]
        Leaf12,
    }

    #[state]
    pub enum Part13 {
        #[initial]
        Leaf13,
    }

    #[state]
    pub enum Part14 {
        #[initial]
        Leaf14,
    }

    #[state]
    pub enum Part15 {
        #[initial]
        Leaf15,
    }

    #[handlers]
    impl Root {}
}

fn main() {}
