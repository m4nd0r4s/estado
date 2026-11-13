//! `cx.raise` and `defer!()` on machines written with `#[machine]`: the
//! queues of spec 5.4 and 5.5, the options `raise_cap`, `defer_cap` and
//! `defer_overflow`, and the detection of `cx.in_state`.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, DeferOverflowPolicy, IgnoreReason, Machine, Next, SendError, Sink, Spec};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Work(u8),
    Done,
    Echo(u8),
    Burst,
    Ping,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, queue::Queue, S>, line: String) {
    cx.ctx_mut().push(line);
}

#[machine(event = Event, ctx = Vec<String>, defer_cap = 2, allow(W0002))]
mod queue {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Queue {
        #[initial]
        Idle,
        Busy,
        Ready,
    }

    #[handlers]
    impl Queue {
        #[entry(Ready)]
        fn entry_ready(cx: &mut Cx<Self>) {
            note(cx, "entry Ready".to_owned());
            // Internal events run before the deferred ones are redelivered.
            cx.raise(Event::Echo(1));
            cx.raise(Event::Echo(2));
        }

        #[on(Idle)]
        fn on_idle(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Idle handles {event:?}"));
            match event {
                Event::Start => goto!(Busy),
                Event::Work(_) => defer!(),
                _ => bubble!(),
            }
        }

        #[on(Busy)]
        fn on_busy(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Busy handles {event:?}"));
            match event {
                Event::Work(_) => defer!(),
                Event::Done => goto!(Ready),
                _ => bubble!(),
            }
        }

        #[on(Ready)]
        fn on_ready(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, format!("Ready handles {event:?}"));
            match event {
                Event::Work(_) | Event::Echo(_) => stay!(),
                _ => bubble!(),
            }
        }
    }
}

pub use queue::Queue;

fn lines(machine: &mut Machine<Queue>) -> Vec<String> {
    std::mem::take(machine.ctx_mut())
}

fn started() -> Machine<Queue> {
    let mut machine = Machine::<Queue>::new(Vec::new());
    machine.start().unwrap();
    machine.ctx_mut().clear();
    machine
}

#[test]
fn deferred_events_are_redelivered_in_order_after_the_internal_ones() {
    let mut machine = started();
    machine.send(Event::Start).unwrap();
    for number in [1, 2] {
        let report = machine.send(Event::Work(number)).unwrap().report;
        assert!(report.handled);
    }
    lines(&mut machine);

    machine.send(Event::Done).unwrap();

    assert_eq!(
        lines(&mut machine),
        [
            "Busy handles Done",
            "entry Ready",
            // The immediate consequences of the transition come first...
            "Ready handles Echo(1)",
            "Ready handles Echo(2)",
            // ...then the deferred events, in the order they were deferred.
            "Ready handles Work(1)",
            "Ready handles Work(2)",
        ]
    );
}

#[test]
fn a_deferred_event_stays_deferred_while_the_configuration_does_not_change() {
    let mut machine = started();
    // `Idle` defers `Work`; nothing changes, so nothing is redelivered.
    machine.send(Event::Work(1)).unwrap();
    machine.send(Event::Ping).unwrap();
    assert_eq!(
        lines(&mut machine),
        ["Idle handles Work(1)", "Idle handles Ping"]
    );

    machine.send(Event::Start).unwrap();
    // The configuration changed: the deferred `Work(1)` is tried again and
    // deferred again by `Busy`.
    assert_eq!(
        lines(&mut machine),
        ["Idle handles Start", "Busy handles Work(1)"]
    );
}

#[test]
fn the_defer_queue_has_the_declared_capacity_and_policy() {
    assert_eq!(<Queue as Spec>::DEFER_OVERFLOW, DeferOverflowPolicy::Error);
    let mut machine = started();
    machine.send(Event::Work(1)).unwrap();
    machine.send(Event::Work(2)).unwrap();

    // `defer_cap = 2`: the third does not fit and the event comes back.
    let error = machine.send(Event::Work(3)).unwrap_err();
    assert_eq!(error, SendError::DeferOverflow(Event::Work(3)));
    assert!(!machine.is_poisoned());
}

#[machine(event = Event, defer_cap = 2, defer_overflow = drop_oldest, allow(W0002))]
mod lossy {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Lossy {
        #[initial]
        Waiting,
        Served,
    }

    #[handlers]
    impl Lossy {
        #[on(Waiting)]
        fn on_waiting(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Work(_) => defer!(),
                Event::Done => goto!(Served),
                _ => bubble!(),
            }
        }

        #[on(Served)]
        fn on_served(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Work(_) => stay!(),
                _ => bubble!(),
            }
        }
    }
}

#[test]
fn drop_oldest_makes_room_by_losing_the_oldest_deferred_event() {
    use lossy::Lossy;
    assert_eq!(
        <Lossy as Spec>::DEFER_OVERFLOW,
        DeferOverflowPolicy::DropOldest
    );
    let mut machine = Machine::<Lossy>::new(());
    for number in 1..=4 {
        machine.send(Event::Work(number)).unwrap();
    }
    let report = machine.send(Event::Done).unwrap().report;
    // Only the two newest survived to be redelivered: one microstep for
    // `Done`, one per redelivered `Work`.
    assert_eq!(report.microsteps, 3);
}

#[machine(event = Event, raise_cap = 1, allow(W0002))]
mod chatty {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Chatty {
        #[initial]
        Quiet,
        Loud,
    }

    #[handlers]
    impl Chatty {
        #[entry(Loud)]
        fn entry_loud(cx: &mut Cx<Self>) {
            cx.raise(Event::Ping);
            cx.raise(Event::Ping);
        }

        #[on(Quiet)]
        fn on_quiet(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Burst => goto!(Loud),
                _ => bubble!(),
            }
        }
    }
}

#[test]
fn raising_more_than_raise_cap_is_an_error_and_poisons_the_machine() {
    let mut machine = Machine::<chatty::Chatty>::new(());
    let error = machine.send(Event::Burst).unwrap_err();
    assert_eq!(error, SendError::RaiseOverflow);
    assert!(machine.is_poisoned());
}

#[test]
fn an_external_event_nobody_handles_is_ignored_and_the_queues_are_untouched() {
    let mut machine = started();
    machine.send(Event::Start).unwrap();
    machine.send(Event::Work(1)).unwrap();
    machine.send(Event::Done).unwrap();
    let report = machine.send(Event::Burst).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
}

#[machine(event = Event, allow(W0002))]
mod watching {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Watching {
        #[initial]
        Looking,
        Seen,
    }

    #[handlers]
    impl Watching {
        #[on(Looking)]
        fn on_looking(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Ping if cx.in_state(Self::SEEN_ID) => stay!(),
                Event::Ping => goto!(Seen),
                _ => bubble!(),
            }
        }
    }
}

#[test]
fn only_machines_that_call_in_state_track_the_active_configuration() {
    const { assert!(<watching::Watching as Spec>::TRACKS_ACTIVE) };
    const { assert!(!<Queue as Spec>::TRACKS_ACTIVE) };
}

#[machine(event = Event, allow(W0002))]
mod helped {
    use super::*;

    #[root]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Helped {
        #[initial]
        Looking,
        Shouting,
        Seen,
    }

    /// Helpers that take the `cx` of a handler may call `in_state` and
    /// `raise` too.
    fn is_seen<S: Sink<()>>(cx: &Cx<'_, Helped, S>) -> bool {
        cx.in_state(Helped::SEEN_ID)
    }

    fn shout<S: Sink<()>>(cx: &mut Cx<'_, Helped, S>) {
        cx.raise(Event::Ping);
    }

    #[handlers]
    impl Helped {
        #[on(Looking)]
        fn on_looking(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Start if is_seen(cx) => stay!(),
                Event::Start => {
                    shout(cx);
                    goto!(Shouting)
                }
                _ => bubble!(),
            }
        }

        #[on(Shouting)]
        fn on_shouting(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Ping => goto!(Seen),
                _ => bubble!(),
            }
        }
    }
}

#[test]
fn helpers_that_call_in_state_or_raise_are_seen_by_the_macro() {
    const { assert!(<helped::Helped as Spec>::TRACKS_ACTIVE) };
    let mut machine = Machine::<helped::Helped>::new(());
    // The raised `Ping` is delivered before `send` returns.
    machine.send(Event::Start).unwrap();
    assert_eq!(machine.path().to_string(), "Seen");
}
