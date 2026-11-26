use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_test_utils::roundtrip;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser)]
#[kirin(type = ArithType)]
#[chumsky(format = "$sample {amplitude}[, {duration}] -> {result_type}")]
struct Sample {
    amplitude: SSAValue,
    duration: Option<SSAValue>,
    #[kirin(type = result_type.clone())]
    result: ResultValue,
    result_type: ArithType,
}

#[test]
fn parses_optional_operand_only_with_its_separator() {
    let operands = &[("a", ArithType::F64), ("d", ArithType::F64)];
    for (input, expected_duration) in [
        ("%r = sample %a -> f64", None),
        ("%r = sample %a, %d -> f64", Some("d")),
    ] {
        let (mut stage, statement) = roundtrip::emit_statement::<Sample>(input, operands);
        let info = statement.get_info(&stage).unwrap();
        let duration = info
            .definition()
            .duration
            .map(|value| value.get_info(&stage).unwrap().name().unwrap());
        assert_eq!(
            duration,
            expected_duration.map(|name| stage.symbol_table().lookup(name).unwrap())
        );
        assert!(
            ParseStatementTextExt::parse_statement(&mut stage, "%other = sample %a, -> f64")
                .is_err()
        );
    }
}
