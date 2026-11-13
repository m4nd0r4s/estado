//! A machine with more `#[always]` states than a 64-bit mask holds: the pending
//! flags span two words and every state is still served, in order.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next};
use estado_macros::machine;

#[derive(Debug)]
pub struct Tick;

/// `S0` .. `S69`; each state but the last has an `#[always]` to the next one.
#[rustfmt::skip]
#[machine(event = Tick, max_microsteps = 200, allow(W0002))]
mod chain {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Chain {
        #[initial] S0,
        S1,
        S2,
        S3,
        S4,
        S5,
        S6,
        S7,
        S8,
        S9,
        S10,
        S11,
        S12,
        S13,
        S14,
        S15,
        S16,
        S17,
        S18,
        S19,
        S20,
        S21,
        S22,
        S23,
        S24,
        S25,
        S26,
        S27,
        S28,
        S29,
        S30,
        S31,
        S32,
        S33,
        S34,
        S35,
        S36,
        S37,
        S38,
        S39,
        S40,
        S41,
        S42,
        S43,
        S44,
        S45,
        S46,
        S47,
        S48,
        S49,
        S50,
        S51,
        S52,
        S53,
        S54,
        S55,
        S56,
        S57,
        S58,
        S59,
        S60,
        S61,
        S62,
        S63,
        S64,
        S65,
        S66,
        S67,
        S68,
        S69,
    }

    #[handlers]
    impl Chain {
        #[always(S0)] fn always_s0(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S1) }
        #[always(S1)] fn always_s1(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S2) }
        #[always(S2)] fn always_s2(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S3) }
        #[always(S3)] fn always_s3(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S4) }
        #[always(S4)] fn always_s4(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S5) }
        #[always(S5)] fn always_s5(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S6) }
        #[always(S6)] fn always_s6(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S7) }
        #[always(S7)] fn always_s7(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S8) }
        #[always(S8)] fn always_s8(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S9) }
        #[always(S9)] fn always_s9(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S10) }
        #[always(S10)] fn always_s10(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S11) }
        #[always(S11)] fn always_s11(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S12) }
        #[always(S12)] fn always_s12(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S13) }
        #[always(S13)] fn always_s13(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S14) }
        #[always(S14)] fn always_s14(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S15) }
        #[always(S15)] fn always_s15(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S16) }
        #[always(S16)] fn always_s16(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S17) }
        #[always(S17)] fn always_s17(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S18) }
        #[always(S18)] fn always_s18(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S19) }
        #[always(S19)] fn always_s19(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S20) }
        #[always(S20)] fn always_s20(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S21) }
        #[always(S21)] fn always_s21(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S22) }
        #[always(S22)] fn always_s22(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S23) }
        #[always(S23)] fn always_s23(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S24) }
        #[always(S24)] fn always_s24(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S25) }
        #[always(S25)] fn always_s25(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S26) }
        #[always(S26)] fn always_s26(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S27) }
        #[always(S27)] fn always_s27(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S28) }
        #[always(S28)] fn always_s28(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S29) }
        #[always(S29)] fn always_s29(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S30) }
        #[always(S30)] fn always_s30(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S31) }
        #[always(S31)] fn always_s31(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S32) }
        #[always(S32)] fn always_s32(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S33) }
        #[always(S33)] fn always_s33(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S34) }
        #[always(S34)] fn always_s34(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S35) }
        #[always(S35)] fn always_s35(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S36) }
        #[always(S36)] fn always_s36(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S37) }
        #[always(S37)] fn always_s37(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S38) }
        #[always(S38)] fn always_s38(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S39) }
        #[always(S39)] fn always_s39(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S40) }
        #[always(S40)] fn always_s40(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S41) }
        #[always(S41)] fn always_s41(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S42) }
        #[always(S42)] fn always_s42(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S43) }
        #[always(S43)] fn always_s43(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S44) }
        #[always(S44)] fn always_s44(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S45) }
        #[always(S45)] fn always_s45(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S46) }
        #[always(S46)] fn always_s46(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S47) }
        #[always(S47)] fn always_s47(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S48) }
        #[always(S48)] fn always_s48(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S49) }
        #[always(S49)] fn always_s49(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S50) }
        #[always(S50)] fn always_s50(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S51) }
        #[always(S51)] fn always_s51(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S52) }
        #[always(S52)] fn always_s52(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S53) }
        #[always(S53)] fn always_s53(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S54) }
        #[always(S54)] fn always_s54(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S55) }
        #[always(S55)] fn always_s55(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S56) }
        #[always(S56)] fn always_s56(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S57) }
        #[always(S57)] fn always_s57(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S58) }
        #[always(S58)] fn always_s58(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S59) }
        #[always(S59)] fn always_s59(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S60) }
        #[always(S60)] fn always_s60(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S61) }
        #[always(S61)] fn always_s61(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S62) }
        #[always(S62)] fn always_s62(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S63) }
        #[always(S63)] fn always_s63(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S64) }
        #[always(S64)] fn always_s64(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S65) }
        #[always(S65)] fn always_s65(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S66) }
        #[always(S66)] fn always_s66(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S67) }
        #[always(S67)] fn always_s67(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S68) }
        #[always(S68)] fn always_s68(_cx: &mut Cx<Self>) -> Next<Self> { goto!(S69) }
    }
}

use chain::Chain;

#[test]
fn every_always_is_served_when_there_are_more_than_64_states() {
    let mut machine = Machine::<Chain>::new(());
    let report = machine.start().unwrap().report;

    // Each `#[always]` is one microstep: S0 -> S1 -> ... -> S69.
    assert_eq!(*machine.state(), Chain::S69);
    assert_eq!(report.microsteps, 69);
    assert_eq!(report.transitions, 69);
}

#[test]
fn the_flags_of_a_machine_that_settled_are_all_clear() {
    let mut machine = Machine::<Chain>::new(());
    machine.start().unwrap();

    // Nothing is pending any more: an event is handled alone.
    let report = machine.send(Tick).unwrap().report;
    assert_eq!(report.microsteps, 1);
    assert_eq!(*machine.state(), Chain::S69);
}
