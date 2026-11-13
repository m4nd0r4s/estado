//! `cx.in_state` inside an `#[always]` handler sees the active configuration,
//! including sibling regions (spec 4.3, 5.9, errata 11).
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    TurnOn,
    Check,
}

#[machine(event = Event, ctx = Vec<bool>, allow(W0001, W0002))]
mod panel {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Panel {
        #[initial]
        Running(Running),
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Running {
        #[region]
        pub light: Light,
        #[region]
        pub mode: Mode,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Light {
        #[initial]
        Off,
        On,
    }

    #[state]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Mode {
        #[initial]
        Waiting,
        Checking,
        Done,
    }

    #[handlers]
    impl Panel {
        #[on(Off)]
        fn on_off(event: &Event) -> Next<Self> {
            match event {
                Event::TurnOn => goto!(On),
                _ => bubble!(),
            }
        }

        #[on(Waiting)]
        fn on_waiting(event: &Event) -> Next<Self> {
            match event {
                Event::Check => goto!(Checking),
                _ => bubble!(),
            }
        }

        /// Records whether the sibling region's `On` is active.
        #[always(Checking)]
        fn always_checking(cx: &mut Cx<Self>) -> Next<Self> {
            let light_is_on = cx.in_state(Panel::ON_ID);
            cx.ctx_mut().push(light_is_on);
            goto!(Done)
        }
    }
}

use panel::Panel;

#[test]
fn in_state_inside_always_sees_sibling_regions() {
    let mut dark = Machine::<Panel>::new(Vec::new());
    dark.send(Event::Check).unwrap();

    let mut lit = Machine::<Panel>::new(Vec::new());
    lit.send(Event::TurnOn).unwrap();
    lit.send(Event::Check).unwrap();

    assert_eq!(*dark.ctx(), [false]);
    assert_eq!(*lit.ctx(), [true]);
}
