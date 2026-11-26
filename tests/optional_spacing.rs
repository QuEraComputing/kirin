use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_test_utils::roundtrip;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(type = ArithType)]
enum Sample {
    #[chumsky(format = "$bare {amplitude}[, {duration}] -> {result_type}")]
    Bare {
        amplitude: SSAValue,
        duration: Vec<SSAValue>,
        result_type: ArithType,
    },
    #[chumsky(format = "$grouped ({amplitude})[, {duration}] -> {result_type}")]
    Grouped {
        amplitude: SSAValue,
        duration: Vec<SSAValue>,
        result_type: ArithType,
    },
}

#[test]
fn prints_optional_comma_without_a_leading_space() {
    let operands = &[("a", ArithType::F64), ("d", ArithType::F64)];
    for input in [
        "bare %a -> f64",
        "bare %a, %d -> f64",
        "grouped (%a) -> f64",
        "grouped (%a), %d -> f64",
    ] {
        let (stage, statement) = roundtrip::emit_statement::<Sample>(input, operands);
        let rendered = roundtrip::render_statement(&stage, statement);
        assert_eq!(rendered.trim(), input);
    }
}
