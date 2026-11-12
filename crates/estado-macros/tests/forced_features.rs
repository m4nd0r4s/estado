//! ADR 0013 item 8: feature detection can be forced. The macro finds
//! `cx.raise(..)` and `cx.in_state(..)` by looking for the method call inside
//! the machine module; a helper outside the module, or a call written
//! `Cx::raise(cx, ..)`, is invisible to it. `#[machine(tracks_active)]` and an
//! explicit `raise_cap` / `defer_cap` switch the features on regardless.
//!
//! Before, such a machine had no queues (the first `raise` overflowed and
//! poisoned it) and `in_state` was false for ever.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Toggle,
    Flash,
    Probe,
}

/// Outside the machine module, and written as a path call: the syntactic scan
/// of the module sees neither.
fn flash<S: Sink<()>>(cx: &mut Cx<'_, lamp::Lamp, S>) {
    Cx::raise(cx, Event::Flash);
}

fn is_on<S: Sink<()>>(cx: &Cx<'_, lamp::Lamp, S>) -> bool {
    Cx::in_state(cx, lamp::Lamp::ON)
}

#[machine(event = Event, ctx = Vec<String>, raise_cap = 2, tracks_active, allow(W0002))]
mod lamp {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lamp {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Lamp {
        #[on(Off)]
        fn off(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Toggle => {
                    flash(cx);
                    goto!(On)
                }
                _ => bubble!(),
            }
        }

        #[on(On)]
        fn on(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Flash => {
                    cx.ctx_mut().push("flash".to_owned());
                    stay!()
                }
                Event::Probe => {
                    let line = if is_on(cx) { "probe: on" } else { "probe: off" };
                    cx.ctx_mut().push(line.to_owned());
                    stay!()
                }
                _ => bubble!(),
            }
        }
    }
}

use lamp::Lamp;

#[test]
fn a_raise_written_outside_the_module_still_has_its_queue() {
    let mut machine = Machine::<Lamp>::new(Vec::new());

    machine.send(Event::Toggle).unwrap();

    assert!(!machine.is_poisoned());
    assert_eq!(machine.ctx(), &["flash".to_owned()]);
}

#[test]
fn in_state_called_from_a_helper_sees_the_active_configuration() {
    let mut machine = Machine::<Lamp>::new(Vec::new());
    machine.send(Event::Toggle).unwrap();
    machine.ctx_mut().clear();

    machine.send(Event::Probe).unwrap();

    assert_eq!(machine.ctx(), &["probe: on".to_owned()]);
}
