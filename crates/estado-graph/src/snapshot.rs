//! File snapshots: compare generated text with a committed file, and
//! rewrite the file on request.
//!
//! The comparison is exact except for line endings (`\r\n` and `\n` are the
//! same), so a checkout with `core.autocrlf` still passes.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::string::String;

use estado_core::Graph;

use crate::Format;

/// The environment variable that turns a failing snapshot into a rewrite.
pub const BLESS_VARIABLE: &str = "ESTADO_BLESS";

/// Whether `ESTADO_BLESS` asks to rewrite snapshots: set, and not empty, `0`
/// or `false`.
pub fn bless_requested() -> bool {
    match std::env::var(BLESS_VARIABLE) {
        Ok(value) => !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false")),
        Err(_) => false,
    }
}

/// What [`check_snapshot`] found.
#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotOutcome {
    /// The file already holds the text.
    Matches,
    /// The file did not match, or did not exist, and was written.
    Written,
}

/// Why a snapshot check failed.
#[derive(Debug)]
pub enum SnapshotError {
    /// The file does not exist and blessing is off.
    Missing(PathBuf),
    /// The file differs from the text.
    Differs {
        /// The snapshot file.
        path: PathBuf,
        /// The 1-based line of the first difference.
        line: usize,
        /// That line in the file (empty when the file is shorter).
        expected: String,
        /// That line in the generated text (empty when the text is shorter).
        actual: String,
    },
    /// Reading or writing the file failed.
    Io(PathBuf, io::Error),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotError::Missing(path) => write!(
                formatter,
                "snapshot `{}` does not exist; run the test with {BLESS_VARIABLE}=1 to create it",
                path.display()
            ),
            SnapshotError::Differs {
                path,
                line,
                expected,
                actual,
            } => write!(
                formatter,
                "snapshot `{}` is out of date; first difference at line {line}:\n  \
                 file:      {expected:?}\n  generated: {actual:?}\n\
                 run the test with {BLESS_VARIABLE}=1 to rewrite it",
                path.display()
            ),
            SnapshotError::Io(path, error) => {
                write!(formatter, "snapshot `{}`: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn first_difference(expected: &str, actual: &str) -> (usize, String, String) {
    let mut expected_lines = expected.split('\n');
    let mut actual_lines = actual.split('\n');
    let mut line = 1;
    loop {
        match (expected_lines.next(), actual_lines.next()) {
            (Some(left), Some(right)) if left == right => line += 1,
            (left, right) => {
                return (
                    line,
                    left.unwrap_or_default().into(),
                    right.unwrap_or_default().into(),
                );
            }
        }
    }
}

/// Compares `actual` with the file at `path`; with `bless` the file is
/// created (with its directories) or rewritten when it differs.
///
/// This is the engine of [`assert_text_snapshot`], without the environment
/// and without panicking.
pub fn check_snapshot(
    path: &Path,
    actual: &str,
    bless: bool,
) -> Result<SnapshotOutcome, SnapshotError> {
    let actual = normalize(actual);
    let existing = match fs::read_to_string(path) {
        Ok(text) => Some(normalize(&text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(SnapshotError::Io(path.to_path_buf(), error)),
    };
    match existing {
        Some(text) if text == actual => Ok(SnapshotOutcome::Matches),
        _ if bless => {
            if let Some(directory) = path.parent() {
                fs::create_dir_all(directory)
                    .map_err(|error| SnapshotError::Io(path.to_path_buf(), error))?;
            }
            fs::write(path, actual)
                .map_err(|error| SnapshotError::Io(path.to_path_buf(), error))?;
            Ok(SnapshotOutcome::Written)
        }
        None => Err(SnapshotError::Missing(path.to_path_buf())),
        Some(text) => {
            let (line, expected, actual) = first_difference(&text, &actual);
            Err(SnapshotError::Differs {
                path: path.to_path_buf(),
                line,
                expected,
                actual,
            })
        }
    }
}

/// Panics unless the file at `path` holds `actual`; rewrites it instead when
/// `ESTADO_BLESS` is set (see [`bless_requested`]).
///
/// `path` is used as given, so a relative path is relative to the working
/// directory of the test, which Cargo sets to the package root.
#[track_caller]
pub fn assert_text_snapshot(path: impl AsRef<Path>, actual: &str) {
    if let Err(error) = check_snapshot(path.as_ref(), actual, bless_requested()) {
        panic!("{error}");
    }
}

/// Renders `graph` and checks it against a snapshot file, for
/// [`assert_snapshot!`](crate::assert_snapshot). Not meant to be called
/// directly: the macro supplies the manifest directory of the calling crate.
#[doc(hidden)]
#[track_caller]
pub fn assert_graph_snapshot(
    graph: &Graph,
    manifest_directory: &str,
    path: &str,
    format: Option<Format>,
) {
    let full_path = Path::new(manifest_directory).join(path);
    let format = format.unwrap_or_else(|| {
        let extension = full_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("");
        Format::from_extension(extension).unwrap_or_else(|| {
            panic!(
                "assert_snapshot!: cannot tell the format of `{path}` from its extension; \
                 use mmd, dot, scxml or json, or pass a Format"
            )
        })
    });
    assert_text_snapshot(full_path, &format.render(graph));
}

/// Asserts that the export of a graph equals a committed file.
///
/// ```ignore
/// estado_graph::assert_snapshot!(Player::GRAPH, "docs/player.mmd");
/// estado_graph::assert_snapshot!(Player::GRAPH, "docs/player.txt", estado_graph::Format::Dot);
/// ```
///
/// The first argument is a `&Graph` or a `Graph` (`Machine::GRAPH`, a
/// `GRAPH` static). The path is relative to the manifest directory of the
/// crate that calls the macro. Without a third argument the format comes from
/// the extension (see [`Format::from_extension`]).
///
/// The assertion fails when the file is missing or differs. Run the tests with
/// `ESTADO_BLESS=1` to create or rewrite the file instead, then review the
/// diff before committing it.
#[macro_export]
macro_rules! assert_snapshot {
    ($graph:expr, $path:expr $(,)?) => {
        $crate::snapshot::assert_graph_snapshot(
            &$graph,
            ::core::env!("CARGO_MANIFEST_DIR"),
            $path,
            ::core::option::Option::None,
        )
    };
    ($graph:expr, $path:expr, $format:expr $(,)?) => {
        $crate::snapshot::assert_graph_snapshot(
            &$graph,
            ::core::env!("CARGO_MANIFEST_DIR"),
            $path,
            ::core::option::Option::Some($format),
        )
    };
}
