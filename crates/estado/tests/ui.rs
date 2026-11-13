//! E0040: the transition macros are stubs outside a `#[machine]` module;
//! E0090: `goto_dyn!` and `edges!` inside a handler are not supported yet.

#[test]
fn transition_macros_outside_a_machine_report_e0040() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
