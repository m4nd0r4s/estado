//! Parallel states on a machine written with `#[machine]` (spec 5.6, ADR
//! 0005 and 0008): sequential evaluation with short-circuit, `in_state`
//! refreshed after a region-internal transition, consumption and deferral.
//! The cases mirror `estado-core/tests/regions.rs`, which runs them on a
//! hand-written machine. The machine is in `support/panel.rs`.
#![allow(dead_code, missing_docs)]

#[path = "support/panel.rs"]
mod panel_machine;

use panel_machine::*;

#[test]
fn entering_a_parallel_state_enters_its_regions_in_declaration_order() {
    let mut machine = off();
    machine.send(Event::Power).unwrap();
    assert_eq!(path(&machine), "Active/Standby");
    lines(&mut machine);

    machine.send(Event::Power).unwrap();

    assert_eq!(
        lines(&mut machine),
        [
            "Standby handles",
            "entry On (volume 3)",
            "entry Dim",
            "entry Mute",
            "entry Clock",
        ]
    );
    assert_eq!(path(&machine), INITIALS);
}

#[test]
fn leaving_a_parallel_state_exits_the_regions_in_reverse_order() {
    let mut machine = on();

    machine.send(Event::Power).unwrap();

    // Every region looked at `Power` first: none wants it, so the parallel
    // state's own handler did. Then the exits, last region first.
    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "Mute handles",
            "Clock handles",
            "On handles",
            "exit Clock",
            "exit Mute",
            "exit Dim",
            "exit On",
        ]
    );
    assert_eq!(path(&machine), "Active/Standby");
}

#[test]
fn an_explicit_path_enters_the_chosen_state_of_a_region() {
    let mut machine = off();

    machine.send(Event::Direct).unwrap();

    assert_eq!(
        lines(&mut machine),
        [
            "Off handles",
            "entry On (volume 5)",
            "entry Bright",
            "entry Mute",
            "entry Clock",
        ]
    );
    assert_eq!(
        path(&machine),
        "Active/On/light:Bright,sound:Mute,screen:Clock"
    );
}

#[test]
fn a_region_sees_the_transition_the_regions_before_it_applied() {
    let mut machine = on();

    machine.send(Event::Toggle).unwrap();

    // `light` goes `Dim -> Bright` first; `sound` asks `cx.in_state(BRIGHT)`
    // and already sees it (errata 6). `screen` does not handle `Toggle`.
    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "exit Dim",
            "entry Bright",
            "Mute handles",
            "exit Mute",
            "entry Beep (0)",
            "Clock handles",
        ]
    );
    assert_eq!(
        path(&machine),
        "Active/On/light:Bright,sound:Beep,screen:Clock"
    );
}

#[test]
fn a_transition_out_of_the_parallel_state_skips_the_remaining_regions() {
    let mut machine = on();

    machine.send(Event::Smash).unwrap();

    // `light` is first and leaves: `sound` and `screen` never see `Smash`,
    // they just exit (reverse order, then the parallel state).
    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "exit Clock",
            "exit Mute",
            "exit Dim",
            "exit On"
        ]
    );
    assert_eq!(path(&machine), "Off");
}

#[test]
fn an_event_no_region_wants_goes_to_the_parallel_state() {
    let mut machine = on();

    let report = machine.send(Event::Noise).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        ["Dim handles", "Mute handles", "Clock handles", "On handles"]
    );
    assert!(report.handled);
    // The handler of `On` wrote into the fields of `On` and of `Active`.
    let Panel::Active(Active {
        uptime,
        mode: Mode::On(On { volume, .. }),
    }) = machine.state()
    else {
        panic!("Active/On expected");
    };
    assert_eq!((*uptime, *volume), (1, 4));
}

#[test]
fn a_region_that_consumes_the_event_hides_it_from_the_parallel_state() {
    let mut machine = on();
    machine.send(Event::Toggle).unwrap();
    lines(&mut machine);

    // `Toggle` again: `light` consumes it. `On` never sees it.
    machine.send(Event::Toggle).unwrap();

    assert!(!lines(&mut machine).contains(&"On handles".to_owned()));
}

#[test]
fn a_deferral_is_dropped_when_another_region_consumes_the_event() {
    // `Park`: `light` (Dim) consumes it with `stay!()`, `sound` (Mute)
    // defers it. The consumption wins and the deferral is lost (spec 5.6
    // items 4 and 5, ADR 0008).
    let mut machine = on();
    let report = machine.send(Event::Park).unwrap().report;
    assert!(report.handled);
    lines(&mut machine);

    machine.send(Event::Toggle).unwrap();

    // The configuration changed, and nothing was redelivered.
    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "exit Dim",
            "entry Bright",
            "Mute handles",
            "exit Mute",
            "entry Beep (0)",
            "Clock handles",
        ]
    );
}

#[test]
fn a_deferral_stands_when_no_region_consumes_the_event_and_is_redelivered() {
    // `light` starts in `Bright`, which does not handle `Park`: only `sound`
    // (Mute) answers, by deferring.
    let mut machine = off();
    machine.send(Event::Direct).unwrap();
    lines(&mut machine);
    let report = machine.send(Event::Park).unwrap().report;
    assert!(report.handled);
    assert_eq!(
        lines(&mut machine),
        ["Bright handles", "Mute handles", "Clock handles"]
    );

    // `light` changes: the deferred `Park` is tried again, and `Dim`
    // consumes it.
    machine.send(Event::Toggle).unwrap();
    assert_eq!(
        lines(&mut machine),
        [
            "Bright handles",
            "exit Bright",
            "entry Dim",
            "Mute handles",
            "Clock handles",
            // Every region looks at the redelivered event: `Dim` consumes it,
            // `Mute` defers it again, and the consumption wins.
            "Dim handles",
            "Mute handles",
            "Clock handles",
        ]
    );
}
