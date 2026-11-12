//! What happens inside the regions of a parallel state (spec 5.6, ADR 0008):
//! `reenter!()`, a constructor edge, an `#[unhandled]` override, a compound
//! state with its own completion, and the graph. The machine is in
//! `support/panel.rs`.
#![allow(dead_code, missing_docs)]

#[path = "support/panel.rs"]
mod panel_machine;

use estado_core::{IgnoreReason, SendError, Spec, StateKind};
use panel_machine::*;

#[test]
fn reenter_restarts_a_state_of_a_region_and_keeps_its_data() {
    let mut machine = on();
    machine.send(Event::Toggle).unwrap();
    machine.send(Event::Toggle).unwrap();
    lines(&mut machine);
    assert_eq!(
        path(&machine),
        "Active/On/light:Dim,sound:Beep,screen:Clock"
    );

    let report = machine.send(Event::Next).unwrap().report;

    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "Beep handles",
            "exit Beep (1)",
            "entry Beep (1)",
            "Clock handles"
        ]
    );
    assert_eq!(report.transitions, 1);
}

#[test]
fn a_constructor_edge_inside_a_region_moves_the_value_of_the_state() {
    let mut machine = on();
    machine.send(Event::Toggle).unwrap();
    lines(&mut machine);

    // `light` goes back to `Dim`, `sound` rebuilds `Beep` from the old one.
    machine.send(Event::Toggle).unwrap();

    assert_eq!(
        lines(&mut machine),
        [
            "Bright handles",
            "exit Bright",
            "entry Dim",
            "Beep handles",
            "exit Beep (0)",
            "entry Beep (1)",
            "Clock handles",
        ]
    );
    let Panel::Active(Active {
        mode: Mode::On(On { sound, .. }),
        ..
    }) = machine.state()
    else {
        panic!("Active/On expected");
    };
    assert_eq!(*sound, Sound::Beep { count: 1 });
}

#[test]
fn a_state_with_an_unhandled_override_inside_a_region_stops_the_event() {
    let mut machine = on();
    machine.send(Event::Toggle).unwrap();
    lines(&mut machine);

    // `Noise` bubbles out of `Beep`, which has `#[unhandled(error)]`: the
    // other regions still look, but nobody wants it and `On` never sees it.
    let error = machine.send(Event::Noise).unwrap_err();

    assert_eq!(error, SendError::Unhandled(Event::Noise));
    assert!(!lines(&mut machine).contains(&"On handles".to_owned()));
}

#[test]
fn a_compound_state_inside_a_region_dispatches_and_completes() {
    let mut machine = on();
    machine.send(Event::Menu).unwrap();
    assert_eq!(
        path(&machine),
        "Active/On/light:Dim,sound:Mute,screen:Menu/Top"
    );
    lines(&mut machine);

    // The leaf, then the compound, then back out of the region.
    machine.send(Event::Back).unwrap();
    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "Mute handles",
            "Top handles",
            "Menu handles",
            "exit Menu",
            "entry Clock",
        ]
    );

    machine.send(Event::Menu).unwrap();
    machine.send(Event::Next).unwrap();
    lines(&mut machine);
    assert_eq!(
        path(&machine),
        "Active/On/light:Dim,sound:Mute,screen:Menu/Inner"
    );

    // `Inner -> Exit` enters a terminal: the completion of `Menu` runs as its
    // own microstep and sends the region back to `Clock`.
    let report = machine.send(Event::Next).unwrap().report;
    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "Mute handles",
            "Inner handles",
            "Menu done (1)",
            "exit Menu",
            "entry Clock",
        ]
    );
    assert_eq!(report.microsteps, 2);
    assert_eq!(path(&machine), INITIALS);
}

#[test]
fn a_handler_of_a_compound_above_the_parallel_state_sees_what_bubbles_out() {
    let mut machine = on();

    machine.send(Event::Reset).unwrap();

    assert_eq!(
        lines(&mut machine),
        [
            "Dim handles",
            "Mute handles",
            "Clock handles",
            "On handles",
            "Active handles",
            "exit Clock",
            "exit Mute",
            "exit Dim",
            "exit On",
        ]
    );
    assert_eq!(path(&machine), "Off");
}

#[test]
fn regions_are_states_of_the_graph() {
    let graph = <Panel as Spec>::GRAPH;
    for region in [Panel::LIGHT, Panel::SOUND, Panel::SCREEN] {
        let info = graph.state(region);
        assert_eq!(info.kind, StateKind::Region);
        assert_eq!(info.parent, Some(Panel::ON));
    }
    assert_eq!(graph.state(Panel::ON).kind, StateKind::Parallel);
    assert_eq!(graph.state(Panel::TOP).parent, Some(Panel::MENU));
    assert_eq!(graph.state(Panel::MENU).parent, Some(Panel::SCREEN));
    assert_eq!(graph.state(Panel::LIGHT).initial, Some(Panel::DIM));
    const { assert!(<Panel as Spec>::TRACKS_ACTIVE) };
}

#[test]
fn an_unhandled_event_is_ignored() {
    let mut machine = on();
    let report = machine.send(Event::Back).unwrap().report;
    assert_eq!(report.ignored, Some(IgnoreReason::Unhandled));
}
