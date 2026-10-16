//! Mistakes in `#[rewrite_rule]` functions that only surface when the
//! generated code compiles, or that the parser reports, must point at the
//! rule's own tokens.

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/*.rs");
}
