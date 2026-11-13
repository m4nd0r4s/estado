//! Names and labels that need escaping, format by format.

use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind};
use estado_graph::{to_dot, to_json, to_mermaid, to_scxml};

/// A graph of top-level atomic states with the given names, and one edge from
/// the first to the second with the given hint.
fn graph_of(names: &[&'static str], hint: Option<&'static str>) -> Graph {
    let states: Vec<StateInfo> = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let index = u16::try_from(index).unwrap();
            StateInfo::new(StateId::new(index), name, None, StateKind::Atomic, 0)
        })
        .collect();
    let edges: Vec<EdgeInfo> = if names.len() > 1 {
        vec![EdgeInfo {
            from: StateId::new(0),
            to: EdgeTarget::State(StateId::new(1)),
            kind: EdgeKind::Normal,
            event_hint: hint,
        }]
    } else {
        Vec::new()
    };
    Graph {
        machine: "M",
        states: Box::leak(states.into_boxed_slice()),
        edges: Box::leak(edges.into_boxed_slice()),
    }
}

#[test]
fn mermaid_ids_are_plain_identifiers() {
    let graph = graph_of(&["1st", "two words", "", "a-b"], None);
    let diagram = to_mermaid(&graph);
    assert!(diagram.contains("state \"1st\" as s_1st\n"), "{diagram}");
    assert!(
        diagram.contains("state \"two words\" as two_words\n"),
        "{diagram}"
    );
    assert!(diagram.contains("state \"\" as state_2\n"), "{diagram}");
    assert!(diagram.contains("state \"a-b\" as a_b\n"), "{diagram}");
}

#[test]
fn mermaid_ids_are_unique_and_avoid_keywords() {
    let graph = graph_of(&["End", "state", "a_b", "a-b", "a b", "Note"], None);
    let diagram = to_mermaid(&graph);
    for (name, id) in [
        ("End", "End_0"),
        ("state", "state_1"),
        ("a-b", "a_b_3"),
        ("a b", "a_b_4"),
        ("Note", "Note_5"),
    ] {
        assert!(
            diagram.contains(&format!("state \"{name}\" as {id}\n")),
            "{name}: {diagram}"
        );
    }
    // A plain name needs no alias.
    assert!(diagram.contains("\n    a_b\n"), "{diagram}");
}

#[test]
fn mermaid_quotes_and_semicolons_cannot_break_a_line() {
    let graph = graph_of(&["Say \"hi\"", "B"], Some("Event::A;\n  Event::B"));
    let diagram = to_mermaid(&graph);
    assert!(diagram.contains("state \"Say #quot;hi#quot;\" as Say__hi_\n"));
    assert!(diagram.contains("Say__hi_ --> B : Event::A, Event::B\n"));
    assert!(!diagram.contains("Event::A;"));
}

#[test]
fn dot_quotes_names_and_labels() {
    let graph = graph_of(&["Say \"hi\"", "back\\slash\nline"], Some("Event::\"x\""));
    let dot = to_dot(&graph);
    assert!(dot.contains("s0 [label=\"Say \\\"hi\\\"\"];\n"), "{dot}");
    assert!(
        dot.contains("s1 [label=\"back\\\\slash\\nline\"];\n"),
        "{dot}"
    );
    assert!(
        dot.contains("s0 -> s1 [label=\"Event::\\\"x\\\"\"];\n"),
        "{dot}"
    );
    assert!(!dot.contains("slash\nline"));
}

#[test]
fn scxml_escapes_markup_in_attributes() {
    let graph = graph_of(&["A", "B"], Some("Event::<&\"'>"));
    let document = to_scxml(&graph);
    assert!(
        document.contains("event=\"Event.&lt;&amp;&quot;&apos;&gt;\""),
        "{document}"
    );
}

#[test]
fn scxml_ids_are_valid_unique_names() {
    let graph = graph_of(&["1st", "a b", "a_b", "a-b", "a.b", "<x>", ""], None);
    let document = to_scxml(&graph);
    let ids: Vec<&str> = document
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<state id=\""))
        .map(|rest| rest.split('"').next().unwrap())
        .collect();
    assert_eq!(ids.len(), 7, "{document}");
    for id in &ids {
        let mut characters = id.chars();
        let first = characters.next().expect("an id is not empty");
        assert!(first.is_ascii_alphabetic() || first == '_', "{id}");
        assert!(
            characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')),
            "{id}"
        );
    }
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "ids repeat: {ids:?}");
}

#[test]
fn scxml_event_descriptors_split_alternatives() {
    let graph = graph_of(&["A", "B"], Some("Event::Move | Event::Stop"));
    assert!(to_scxml(&graph).contains("event=\"Event.Move Event.Stop\""));
}

#[test]
fn scxml_control_characters_become_the_replacement_character() {
    let graph = graph_of(&["A", "B"], Some("Event::\u{1}x"));
    let document = to_scxml(&graph);
    assert!(document.contains("Event.\u{FFFD}x"));
    assert!(!document.contains('\u{1}'));
}

#[test]
fn json_strings_round_trip() {
    let nasty = "q\" b\\ n\n r\r t\t c\u{1} é 日本";
    let graph = graph_of(&[nasty, "B"], Some(nasty));
    let parsed: serde_json::Value = serde_json::from_str(&to_json(&graph)).unwrap();
    assert_eq!(parsed["states"][0]["name"], nasty);
    assert_eq!(parsed["edges"][0]["event"], nasty);
    assert!(!to_json(&graph).contains('\u{1}'));
}

#[test]
fn an_empty_graph_exports_in_every_format() {
    let graph = graph_of(&[], None);
    assert_eq!(to_mermaid(&graph), "stateDiagram-v2\n    %% machine M\n");
    assert!(to_dot(&graph).ends_with("}\n"));
    assert!(to_scxml(&graph).contains("<scxml "));
    assert!(!to_scxml(&graph).contains("initial="));
    let parsed: serde_json::Value = serde_json::from_str(&to_json(&graph)).unwrap();
    assert_eq!(parsed["states"], serde_json::json!([]));
    assert_eq!(parsed["edges"], serde_json::json!([]));
}
