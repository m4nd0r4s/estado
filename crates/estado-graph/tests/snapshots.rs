//! Snapshots of every format for a graph that uses every kind of state,
//! edge and target. Rewrite them with `ESTADO_BLESS=1 cargo test -p estado-graph`.

mod support;

use estado_graph::{Format, GraphExport, assert_snapshot};
use support::sample::SAMPLE;

#[test]
fn mermaid_snapshot() {
    assert_snapshot!(SAMPLE, "tests/snapshots/sample.mmd");
}

#[test]
fn dot_snapshot() {
    assert_snapshot!(SAMPLE, "tests/snapshots/sample.dot");
}

#[test]
fn scxml_snapshot() {
    assert_snapshot!(SAMPLE, "tests/snapshots/sample.scxml");
}

#[test]
fn json_snapshot() {
    assert_snapshot!(SAMPLE, "tests/snapshots/sample.json");
}

#[test]
fn the_format_can_be_given_explicitly() {
    assert_snapshot!(&SAMPLE, "tests/snapshots/sample.mmd", Format::Mermaid);
}

#[test]
fn output_is_deterministic() {
    for format in Format::ALL {
        assert_eq!(format.render(&SAMPLE), format.render(&SAMPLE));
    }
    assert_eq!(SAMPLE.to_mermaid(), estado_graph::to_mermaid(&SAMPLE));
    assert_eq!(SAMPLE.to_dot(), estado_graph::to_dot(&SAMPLE));
    assert_eq!(SAMPLE.to_scxml(), estado_graph::to_scxml(&SAMPLE));
    assert_eq!(SAMPLE.to_json(), estado_graph::to_json(&SAMPLE));
}
