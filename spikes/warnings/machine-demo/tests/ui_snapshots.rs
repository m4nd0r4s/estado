//! AC-17 strategy for warnings: promote the warning to an error with
//! `#![deny(deprecated)]` inside the fixture, so `trybuild` can snapshot it.

#[test]
fn warning_snapshots() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/w0001_emits.rs");
    cases.pass("tests/ui/w0001_allow_silences.rs");
}
