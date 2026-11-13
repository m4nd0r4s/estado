//! The snapshot engine behind `assert_snapshot!`, without the environment.

use std::fs;
use std::path::PathBuf;

use estado_graph::snapshot::{SnapshotError, SnapshotOutcome, check_snapshot};

/// A fresh scratch directory for one test.
fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("estado-graph-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn a_matching_file_passes_and_is_left_alone() {
    let directory = scratch("match");
    let path = directory.join("a.txt");
    fs::write(&path, "one\ntwo\n").unwrap();
    assert_eq!(
        check_snapshot(&path, "one\ntwo\n", false).unwrap(),
        SnapshotOutcome::Matches
    );
    assert_eq!(
        check_snapshot(&path, "one\ntwo\n", true).unwrap(),
        SnapshotOutcome::Matches
    );
}

#[test]
fn line_endings_do_not_matter() {
    let directory = scratch("crlf");
    let path = directory.join("a.txt");
    fs::write(&path, "one\r\ntwo\r\n").unwrap();
    assert_eq!(
        check_snapshot(&path, "one\ntwo\n", false).unwrap(),
        SnapshotOutcome::Matches
    );
}

#[test]
fn a_missing_file_fails_unless_blessed() {
    let directory = scratch("missing");
    let path = directory.join("nested").join("a.txt");
    assert!(matches!(
        check_snapshot(&path, "text\n", false),
        Err(SnapshotError::Missing(_))
    ));
    assert!(!path.exists());
    assert_eq!(
        check_snapshot(&path, "text\n", true).unwrap(),
        SnapshotOutcome::Written
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "text\n");
}

#[test]
fn a_different_file_reports_the_first_difference_and_is_rewritten_when_blessed() {
    let directory = scratch("differs");
    let path = directory.join("a.txt");
    fs::write(&path, "one\ntwo\nthree\n").unwrap();
    match check_snapshot(&path, "one\nTWO\nthree\n", false) {
        Err(SnapshotError::Differs {
            line,
            expected,
            actual,
            ..
        }) => {
            assert_eq!(line, 2);
            assert_eq!(expected, "two");
            assert_eq!(actual, "TWO");
        }
        other => panic!("expected a difference, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), "one\ntwo\nthree\n");
    let message = check_snapshot(&path, "one\nTWO\nthree\n", false)
        .unwrap_err()
        .to_string();
    assert!(message.contains("ESTADO_BLESS=1"), "{message}");
    assert_eq!(
        check_snapshot(&path, "one\nTWO\nthree\n", true).unwrap(),
        SnapshotOutcome::Written
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "one\nTWO\nthree\n");
}

#[test]
fn a_shorter_text_differs_at_the_first_missing_line() {
    let directory = scratch("shorter");
    let path = directory.join("a.txt");
    fs::write(&path, "one\ntwo\n").unwrap();
    match check_snapshot(&path, "one\n", false) {
        Err(SnapshotError::Differs {
            line,
            expected,
            actual,
            ..
        }) => {
            assert_eq!(line, 2);
            assert_eq!(expected, "two");
            assert_eq!(actual, "");
        }
        other => panic!("expected a difference, got {other:?}"),
    }
}
