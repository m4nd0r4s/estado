use estado_macros::machine;

pub struct Ev;

#[machine(event = Ev)]
mod deep {
    #[root]
    pub enum Root {
        #[initial]
        L0(S0),
    }

    #[state]
    pub struct S0 {
        #[substates]
        sub: E0,
    }

    #[state]
    pub enum E0 {
        #[initial]
        L1(S1),
    }

    #[state]
    pub struct S1 {
        #[substates]
        sub: E1,
    }

    #[state]
    pub enum E1 {
        #[initial]
        L2(S2),
    }

    #[state]
    pub struct S2 {
        #[substates]
        sub: E2,
    }

    #[state]
    pub enum E2 {
        #[initial]
        L3(S3),
    }

    #[state]
    pub struct S3 {
        #[substates]
        sub: E3,
    }

    #[state]
    pub enum E3 {
        #[initial]
        L4(S4),
    }

    #[state]
    pub struct S4 {
        #[substates]
        sub: E4,
    }

    #[state]
    pub enum E4 {
        #[initial]
        L5(S5),
    }

    #[state]
    pub struct S5 {
        #[substates]
        sub: E5,
    }

    #[state]
    pub enum E5 {
        #[initial]
        L6(S6),
    }

    #[state]
    pub struct S6 {
        #[substates]
        sub: E6,
    }

    #[state]
    pub enum E6 {
        #[initial]
        L7(S7),
    }

    #[state]
    pub struct S7 {
        #[substates]
        sub: E7,
    }

    #[state]
    pub enum E7 {
        #[initial]
        L8(S8),
    }

    #[state]
    pub struct S8 {
        #[substates]
        sub: E8,
    }

    #[state]
    pub enum E8 {
        #[initial]
        L9(S9),
    }

    #[state]
    pub struct S9 {
        #[substates]
        sub: E9,
    }

    #[state]
    pub enum E9 {
        #[initial]
        L10(S10),
    }

    #[state]
    pub struct S10 {
        #[substates]
        sub: E10,
    }

    #[state]
    pub enum E10 {
        #[initial]
        L11(S11),
    }

    #[state]
    pub struct S11 {
        #[substates]
        sub: E11,
    }

    #[state]
    pub enum E11 {
        #[initial]
        L12(S12),
    }

    #[state]
    pub struct S12 {
        #[substates]
        sub: E12,
    }

    #[state]
    pub enum E12 {
        #[initial]
        L13(S13),
    }

    #[state]
    pub struct S13 {
        #[substates]
        sub: E13,
    }

    #[state]
    pub enum E13 {
        #[initial]
        L14(S14),
    }

    #[state]
    pub struct S14 {
        #[substates]
        sub: E14,
    }

    #[state]
    pub enum E14 {
        #[initial]
        L15(S15),
    }

    #[state]
    pub struct S15 {
        #[substates]
        sub: E15,
    }

    #[state]
    pub enum E15 {
        #[initial]
        L16(S16),
    }

    #[state]
    pub struct S16 {
        #[substates]
        sub: E16,
    }

    #[state]
    pub enum E16 {
        #[initial]
        Leaf,
    }

}

fn main() {}
