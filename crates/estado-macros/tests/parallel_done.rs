//! Completion of a parallel state (ADR 0005): `#[on_done]` on a parallel
//! state runs when every region is in a `#[terminal]` state, and not before;
//! `reenter!()` on the parallel state restarts every region.
#![allow(dead_code, missing_docs)]

use estado_core::{Cx, Machine, Next, Sink};
use estado_macros::machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Sent,
    Verified,
    Retry,
}

fn note<S: Sink<()>>(cx: &mut Cx<'_, handshake::Handshake, S>, line: &str) {
    cx.ctx_mut().push(line.to_owned());
}

#[machine(event = Event, ctx = Vec<String>)]
mod handshake {
    use super::*;

    #[root]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Handshake {
        #[initial]
        Idle,
        Upload(Upload),
        #[terminal]
        Finished,
    }

    #[state]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Upload {
        pub name: String,
        pub rounds: u8,
        #[region]
        pub transfer: Transfer,
        #[region]
        pub check: Check,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Transfer {
        #[initial]
        Sending,
        #[terminal]
        Sent,
    }

    #[state]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Check {
        #[initial]
        Checking,
        #[terminal]
        Verified,
    }

    #[handlers]
    impl Handshake {
        #[entry(Sending)]
        fn entry_sending(cx: &mut Cx<Self>) {
            note(cx, "entry Sending");
        }

        #[entry(Checking)]
        fn entry_checking(cx: &mut Cx<Self>) {
            note(cx, "entry Checking");
        }

        #[entry(Sent)]
        fn entry_sent(cx: &mut Cx<Self>) {
            note(cx, "entry Sent");
        }

        #[entry(Verified)]
        fn entry_verified(cx: &mut Cx<Self>) {
            note(cx, "entry Verified");
        }

        #[exit(Upload)]
        fn exit_upload(up: UploadFields<'_>, cx: &mut Cx<Self>) {
            note(cx, &format!("exit Upload {}", up.name));
        }

        #[on(Idle)]
        fn on_idle(_cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            match event {
                Event::Start => goto!(Upload {
                    name: "report".to_owned(),
                    rounds: 0,
                }),
                _ => bubble!(),
            }
        }

        #[on(Sending)]
        fn on_sending(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Sending handles");
            match event {
                Event::Sent => goto!(Sent),
                _ => bubble!(),
            }
        }

        #[on(Checking)]
        fn on_checking(cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Checking handles");
            match event {
                Event::Verified => goto!(Verified),
                _ => bubble!(),
            }
        }

        /// `reenter!()` restarts both regions and keeps the shared data.
        #[on(Upload)]
        fn on_upload(up: UploadFields<'_>, cx: &mut Cx<Self>, event: &Event) -> Next<Self> {
            note(cx, "Upload handles");
            match event {
                Event::Retry => {
                    *up.rounds += 1;
                    reenter!()
                }
                _ => bubble!(),
            }
        }

        /// No `Output` parameter: each region may end in another terminal.
        #[on_done(Upload)]
        fn upload_done(up: UploadFields<'_>, cx: &mut Cx<Self>) -> Next<Self> {
            note(cx, &format!("on_done Upload ({} rounds)", up.rounds));
            goto!(Finished)
        }
    }
}

pub use handshake::Handshake;

fn lines(machine: &mut Machine<Handshake>) -> Vec<String> {
    std::mem::take(machine.ctx_mut())
}

fn uploading() -> Machine<Handshake> {
    let mut machine = Machine::<Handshake>::new(Vec::new());
    machine.send(Event::Start).unwrap();
    lines(&mut machine);
    machine
}

#[test]
fn partial_completion_fires_nothing() {
    let mut machine = uploading();

    let report = machine.send(Event::Sent).unwrap().report;

    assert_eq!(
        machine.path().to_string(),
        "Upload/transfer:Sent,check:Checking"
    );
    assert_eq!(
        lines(&mut machine),
        ["Sending handles", "entry Sent", "Checking handles"]
    );
    // One microstep for the event, one for the completion check, which found
    // one region still running.
    assert_eq!(report.microsteps, 2);
    assert!(!machine.is_done());
}

#[test]
fn the_parallel_state_completes_when_the_last_region_enters_a_terminal() {
    let mut machine = uploading();
    machine.send(Event::Sent).unwrap();
    lines(&mut machine);

    let report = machine.send(Event::Verified).unwrap().report;

    // The finished `transfer` has no handler: only `check` runs. Then the
    // completion microstep: `on_done` leaves, all regions exit (reverse
    // order), then `Upload` itself.
    assert_eq!(
        lines(&mut machine),
        [
            "Checking handles",
            "entry Verified",
            "on_done Upload (0 rounds)",
            "exit Upload report",
        ]
    );
    assert_eq!(report.microsteps, 2);
    assert_eq!(report.transitions, 2);
    assert!(machine.is_done());
    assert_eq!(machine.path().to_string(), "Finished");
}

#[test]
fn reenter_on_a_parallel_state_restarts_every_region() {
    let mut machine = uploading();
    machine.send(Event::Sent).unwrap();
    lines(&mut machine);

    machine.send(Event::Retry).unwrap();

    assert_eq!(
        machine.path().to_string(),
        "Upload/transfer:Sending,check:Checking"
    );
    assert_eq!(
        lines(&mut machine),
        [
            "Checking handles",
            "Upload handles",
            "exit Upload report",
            "entry Sending",
            "entry Checking",
        ]
    );
    // The shared data survived, and `on_done` sees it later.
    machine.send(Event::Sent).unwrap();
    machine.send(Event::Verified).unwrap();
    assert!(lines(&mut machine).contains(&"on_done Upload (1 rounds)".to_owned()));
}
