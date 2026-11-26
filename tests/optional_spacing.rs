use kirin_arith::ArithType;
use kirin_test_languages::OptionalSpacingLanguage;
use kirin_test_utils::roundtrip;

#[test]
fn prints_optional_comma_without_a_leading_space() {
    let operands = &[("a", ArithType::F64), ("d", ArithType::F64)];
    for input in [
        "bare %a -> f64",
        "bare %a, %d -> f64",
        "grouped (%a) -> f64",
        "grouped (%a), %d -> f64",
    ] {
        let (stage, statement) =
            roundtrip::emit_statement::<OptionalSpacingLanguage>(input, operands);
        let rendered = roundtrip::render_statement(&stage, statement);
        assert_eq!(rendered.trim(), input);
    }
}

#[test]
fn keeps_adjacent_closing_braces_separate() {
    let operands = &[("a", ArithType::F64), ("b", ArithType::F64)];
    for input in [
        "close_before} } %a",
        "close_after} }",
        "close_chain} }",
        "close_chain}, %b}",
    ] {
        let (stage, statement) =
            roundtrip::emit_statement::<OptionalSpacingLanguage>(input, operands);
        let rendered = roundtrip::render_statement(&stage, statement);
        assert_eq!(rendered.trim(), input);
    }
}
