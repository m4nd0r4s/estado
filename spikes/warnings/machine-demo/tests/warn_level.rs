//! Proves the diagnostic really is a `warning` (not an error) at the default
//! lint level, that its span is precise, and that cargo's cap-lints (applied
//! to non-workspace dependencies) hides it.

use std::process::Command;

struct Diagnostic {
    level: String,
    message: String,
    line_start: u64,
    column_start: u64,
}

fn check_example(example: &str, extra_rustflags: &str) -> Vec<Diagnostic> {
    let target_dir = format!("{}/warn-level-target", env!("CARGO_TARGET_TMPDIR"));
    let output = Command::new(env!("CARGO"))
        .args([
            "check",
            "--offline",
            "--message-format=json",
            "--example",
            example,
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTFLAGS", extra_rustflags)
        .output()
        .expect("cargo check must run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| value["reason"] == "compiler-message")
        .filter_map(|value| {
            let message = &value["message"];
            let span = message["spans"].as_array()?.first()?;
            Some(Diagnostic {
                level: message["level"].as_str()?.to_owned(),
                message: message["message"].as_str()?.to_owned(),
                line_start: span["line_start"].as_u64()?,
                column_start: span["column_start"].as_u64()?,
            })
        })
        .filter(|diagnostic| diagnostic.message.contains("estado::W0001"))
        .collect()
}

#[test]
fn warning_has_warn_level_and_precise_span_once_per_occurrence() {
    let diagnostics = check_example("probe", "");
    assert_eq!(diagnostics.len(), 2, "one warning per occurrence");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.level == "warning")
    );
    let positions: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.line_start, diagnostic.column_start))
        .collect();
    assert_eq!(
        positions,
        [(9, 5), (12, 5)],
        "span points at the variant identifiers"
    );
    assert!(diagnostics[0].message.contains("`Orphan`"));
}

#[test]
fn warning_is_hidden_by_cap_lints_like_in_a_dependency() {
    // Cargo passes `--cap-lints allow` to every non-workspace dependency.
    assert!(check_example("probe", "--cap-lints=allow").is_empty());
}

#[test]
fn user_allow_deprecated_scopes() {
    let diagnostics = check_example("user_allow_deprecated", "");
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    // On the enum: NOT hidden. On the enclosing module: hidden.
    // In a function body: precise span. Via allow(W0001): not emitted.
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`OnEnumOrphan`"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`InFunctionOrphan`"))
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.contains("`InModuleOrphan`"))
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.contains("`SilencedOrphan`"))
    );
}
