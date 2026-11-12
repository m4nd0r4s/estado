//! The throwaway crates the machines are checked in, and the timing of
//! `cargo check` on them. Everything lives under `<repository>/target/bm05`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// A crate that depends on `estado-core` and `estado-macros` by path.
pub struct Probe {
    directory: PathBuf,
    target_directory: PathBuf,
}

impl Probe {
    /// Writes the crate `name` with `source` as its `lib.rs`.
    pub fn write(work_directory: &Path, repository: &Path, name: &str, source: &str) -> Self {
        let directory = work_directory.join(name);
        fs::create_dir_all(directory.join("src")).expect("create the probe directory");
        let manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
             # Standalone: never part of the estado workspace.\n[workspace]\n\n\
             [dependencies]\nestado-core = {{ path = \"{core}\", features = [\"std\"] }}\n\
             estado-macros = {{ path = \"{macros}\" }}\n",
            core = repository.join("crates/estado-core").display(),
            macros = repository.join("crates/estado-macros").display(),
        );
        fs::write(directory.join("Cargo.toml"), manifest).expect("write Cargo.toml");
        // Pin the dependency versions to the ones the repository tests.
        let lockfile = repository.join("Cargo.lock");
        if lockfile.exists() {
            fs::copy(lockfile, directory.join("Cargo.lock")).expect("copy Cargo.lock");
        }
        fs::write(directory.join("src/lib.rs"), source).expect("write lib.rs");
        Self {
            directory,
            target_directory: work_directory.join("target"),
        }
    }

    /// Rewrites `lib.rs` with the same text, so cargo sees a changed file.
    pub fn touch(&self, source: &str) {
        fs::write(self.directory.join("src/lib.rs"), source).expect("rewrite lib.rs");
    }

    /// One `cargo check` of this crate. Incremental compilation is off:
    /// the cached query results would hide the type-checking work.
    pub fn check(&self) -> Result<Duration, String> {
        let started = Instant::now();
        let output = Command::new("cargo")
            .args(["check", "--quiet", "--manifest-path"])
            .arg(self.directory.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", &self.target_directory)
            .env("CARGO_INCREMENTAL", "0")
            .output()
            .map_err(|error| format!("cannot run cargo: {error}"))?;
        let elapsed = started.elapsed();
        if output.status.success() {
            Ok(elapsed)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}

/// The median of the durations (the slowest outlier never decides a run).
pub fn median(mut durations: Vec<Duration>) -> Duration {
    durations.sort();
    durations[durations.len() / 2]
}
