//! E0040: the transition macros are stubs outside a `#[machine]` module.

#[test]
fn transition_macros_outside_a_machine_report_e0040() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
