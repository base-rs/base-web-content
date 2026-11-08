// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

#[test]
fn invalid_declarations_are_refused_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
