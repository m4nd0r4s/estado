//! Warnings of the macros (spec 6.7, ADR 0003, AC-17): each fixture has
//! `#![deny(deprecated)]`, which turns the warning into an error `trybuild`
//! can snapshot.

#[test]
fn warnings_are_emitted_and_silenced() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/warnings/*_emits.rs");
    cases.compile_fail("tests/warnings/*_counts_own_fields.rs");
    cases.pass("tests/warnings/*_silences.rs");
    cases.pass("tests/warnings/*_below_the_limit.rs");
    cases.pass("tests/warnings/*_clean_machine.rs");
    cases.pass("tests/warnings/*_no_warning.rs");
}
