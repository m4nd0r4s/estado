//! The JSON export against `docs/graph.schema.json`.

mod support;

use estado_graph::to_json;
use serde_json::{Value, json};
use support::sample::SAMPLE;
use support::schema::{graph_schema, unsupported_keywords, validate};

#[test]
fn the_validator_knows_every_keyword_of_the_schema() {
    let mut unknown = Vec::new();
    unsupported_keywords(&graph_schema(), &mut unknown);
    assert!(
        unknown.is_empty(),
        "teach tests/support/schema.rs these keywords: {unknown:?}"
    );
}

#[test]
fn the_sample_export_satisfies_the_schema() {
    let instance: Value = serde_json::from_str(&to_json(&SAMPLE)).unwrap();
    if let Err(errors) = validate(&graph_schema(), &instance) {
        panic!("the export breaks the schema:\n{}", errors.join("\n"));
    }
}

#[test]
fn the_committed_example_exports_satisfy_the_schema() {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/graphs");
    let mut checked = 0;
    for entry in std::fs::read_dir(directory).expect("docs/graphs exists") {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let text = std::fs::read_to_string(&path).unwrap();
            let instance: Value = serde_json::from_str(&text).unwrap();
            if let Err(errors) = validate(&graph_schema(), &instance) {
                panic!(
                    "{} breaks the schema:\n{}",
                    path.display(),
                    errors.join("\n")
                );
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 4,
        "expected one JSON file per example, found {checked}"
    );
}

#[test]
fn the_schema_rejects_what_it_should() {
    let schema = graph_schema();
    let valid: Value = serde_json::from_str(&to_json(&SAMPLE)).unwrap();

    let mut wrong_version = valid.clone();
    wrong_version["version"] = json!(2);
    assert!(validate(&schema, &wrong_version).is_err());

    let mut extra_property = valid.clone();
    extra_property["extra"] = json!(true);
    assert!(validate(&schema, &extra_property).is_err());

    let mut unknown_kind = valid.clone();
    unknown_kind["states"][0]["kind"] = json!("simple");
    assert!(validate(&schema, &unknown_kind).is_err());

    let mut negative_id = valid.clone();
    negative_id["states"][1]["parent"] = json!(-1);
    assert!(validate(&schema, &negative_id).is_err());

    let mut missing_event = valid.clone();
    missing_event["edges"][0]
        .as_object_mut()
        .unwrap()
        .remove("event");
    assert!(validate(&schema, &missing_event).is_err());

    let mut bad_target = valid.clone();
    bad_target["edges"][0]["to"] = json!({ "type": "dynamic", "id": 3 });
    assert!(validate(&schema, &bad_target).is_err());

    let mut unknown_target = valid;
    unknown_target["edges"][0]["to"] = json!({ "type": "nowhere" });
    assert!(validate(&schema, &unknown_target).is_err());
}

#[test]
fn json_follows_the_graph() {
    let instance: Value = serde_json::from_str(&to_json(&SAMPLE)).unwrap();
    assert_eq!(instance["machine"], SAMPLE.machine);
    assert_eq!(
        instance["states"].as_array().unwrap().len(),
        SAMPLE.states.len()
    );
    assert_eq!(
        instance["edges"].as_array().unwrap().len(),
        SAMPLE.edges.len()
    );
    for (index, state) in SAMPLE.states.iter().enumerate() {
        assert_eq!(instance["states"][index]["id"], state.id.index());
        assert_eq!(instance["states"][index]["name"], state.name);
        assert_eq!(
            instance["states"][index]["parent"],
            state
                .parent
                .map_or(Value::Null, |parent| json!(parent.index()))
        );
    }
}
