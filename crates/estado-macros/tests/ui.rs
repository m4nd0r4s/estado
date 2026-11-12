//! Diagnostics of the macros (spec 6.7): one `trybuild` snapshot per error.

#[test]
fn diagnostics_have_codes_spans_and_help() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
