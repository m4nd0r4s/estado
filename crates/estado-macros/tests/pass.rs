//! Machines that must compile cleanly under strict lints of the user crate.

#[test]
fn machines_compile_under_strict_lints() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/pass/*.rs");
}
