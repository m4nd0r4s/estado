//! A small JSON Schema validator for the subset of keywords that
//! `docs/graph.schema.json` uses: `type`, `enum`, `const`, `properties`,
//! `required`, `additionalProperties: false`, `items`, `minimum`, `maximum`,
//! `oneOf` and local `$ref`. Anything else in the schema is ignored, so a new
//! keyword in the schema needs a new case here (the tests check the schema
//! uses no keyword outside this list).

use serde_json::Value;

/// The keywords this validator understands, besides annotations.
const SUPPORTED: [&str; 12] = [
    "type",
    "enum",
    "const",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "minimum",
    "maximum",
    "oneOf",
    "$ref",
    "$defs",
];

/// Annotation keywords that carry no constraint.
const ANNOTATIONS: [&str; 4] = ["$schema", "$id", "title", "description"];

/// The schema as committed in `docs/`.
pub fn graph_schema() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/graph.schema.json");
    let text = std::fs::read_to_string(path).expect("docs/graph.schema.json exists");
    serde_json::from_str(&text).expect("the schema is valid JSON")
}

/// Keywords of `schema` (and of every subschema) this validator does not know.
pub fn unsupported_keywords(schema: &Value, found: &mut Vec<String>) {
    let Value::Object(map) = schema else { return };
    for (keyword, value) in map {
        if !SUPPORTED.contains(&keyword.as_str()) && !ANNOTATIONS.contains(&keyword.as_str()) {
            found.push(keyword.clone());
        }
        match keyword.as_str() {
            "properties" | "$defs" => {
                if let Value::Object(children) = value {
                    for child in children.values() {
                        unsupported_keywords(child, found);
                    }
                }
            }
            "items" => unsupported_keywords(value, found),
            "oneOf" => {
                if let Value::Array(children) = value {
                    for child in children {
                        unsupported_keywords(child, found);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Validates `instance` against `schema`; the errors name the JSON path.
pub fn validate(root: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    check(root, root, instance, "$", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn type_matches(name: &str, instance: &Value) -> bool {
    match name {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "null" => instance.is_null(),
        "boolean" => instance.is_boolean(),
        "integer" => instance.is_i64() || instance.is_u64(),
        "number" => instance.is_number(),
        other => panic!("unknown type `{other}` in the schema"),
    }
}

fn check(root: &Value, schema: &Value, instance: &Value, path: &str, errors: &mut Vec<String>) {
    let Value::Object(map) = schema else {
        panic!("a schema is an object, found {schema} at {path}");
    };
    if let Some(Value::String(reference)) = map.get("$ref") {
        let pointer = reference
            .strip_prefix('#')
            .unwrap_or_else(|| panic!("only local $ref is supported: {reference}"));
        let target = root
            .pointer(pointer)
            .unwrap_or_else(|| panic!("unresolved $ref {reference}"));
        check(root, target, instance, path, errors);
    }
    if let Some(expected) = map.get("type") {
        let allowed: Vec<&str> = match expected {
            Value::String(name) => vec![name.as_str()],
            Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
            other => panic!("bad type keyword {other}"),
        };
        if !allowed.iter().any(|name| type_matches(name, instance)) {
            errors.push(format!("{path}: expected {allowed:?}, found {instance}"));
            return;
        }
    }
    if let Some(Value::Array(values)) = map.get("enum") {
        if !values.contains(instance) {
            errors.push(format!("{path}: {instance} is not one of {values:?}"));
        }
    }
    if let Some(expected) = map.get("const") {
        if expected != instance {
            errors.push(format!("{path}: expected {expected}, found {instance}"));
        }
    }
    if let Some(number) = instance.as_i64() {
        if let Some(minimum) = map.get("minimum").and_then(Value::as_i64) {
            if number < minimum {
                errors.push(format!("{path}: {number} is below {minimum}"));
            }
        }
        if let Some(maximum) = map.get("maximum").and_then(Value::as_i64) {
            if number > maximum {
                errors.push(format!("{path}: {number} is above {maximum}"));
            }
        }
    }
    if let Value::Object(members) = instance {
        if let Some(Value::Array(required)) = map.get("required") {
            for name in required.iter().filter_map(Value::as_str) {
                if !members.contains_key(name) {
                    errors.push(format!("{path}: missing property `{name}`"));
                }
            }
        }
        let properties = map.get("properties").and_then(Value::as_object);
        for (name, value) in members {
            match properties.and_then(|properties| properties.get(name)) {
                Some(subschema) => {
                    check(root, subschema, value, &format!("{path}.{name}"), errors);
                }
                None if map.get("additionalProperties") == Some(&Value::Bool(false)) => {
                    errors.push(format!("{path}: unexpected property `{name}`"));
                }
                None => {}
            }
        }
    }
    if let (Value::Array(items), Some(subschema)) = (instance, map.get("items")) {
        for (index, item) in items.iter().enumerate() {
            check(root, subschema, item, &format!("{path}[{index}]"), errors);
        }
    }
    if let Some(Value::Array(alternatives)) = map.get("oneOf") {
        let matching = alternatives
            .iter()
            .filter(|alternative| {
                let mut scratch = Vec::new();
                check(root, alternative, instance, path, &mut scratch);
                scratch.is_empty()
            })
            .count();
        if matching != 1 {
            errors.push(format!(
                "{path}: {matching} oneOf alternatives match, expected 1"
            ));
        }
    }
}
