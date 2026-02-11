use kirin::ir::{Dialect, Pipeline, Region, StageInfo};
use kirin::parsers::{HasParser, ParsePipelineText, PrettyPrint};
use kirin::pretty::FunctionPrintExt;
use kirin_arith::ArithType;
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum BitwiseFunctionLanguage {
    #[chumsky(format = "{body}")]
    Function { body: Region },
    #[wraps]
    Bitwise(Bitwise<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
}

#[test]
fn test_bitwise_function_roundtrip_print_parse_print() {
    let input = r#"
stage @bitwise fn @compose(i64, i64, u32, u32) -> i64;
specialize @bitwise fn @compose(i64, i64, u32, u32) -> i64 {
  ^entry(%a: i64, %b: i64, %x: u32, %y: u32) {
    %and = and %a, %b -> i64;
    %or = or %and, %b -> i64;
    %xor = xor %or, %a -> i64;
    %not = not %xor -> i64;
    %shl = shl %x, %y -> u32;
    %shr = shr %shl, %y -> u32;
    ret %not;
  }
}
"#;

    let mut parsed_pipeline: Pipeline<StageInfo<BitwiseFunctionLanguage>> = Pipeline::new();
    let parsed_functions = parsed_pipeline
        .parse(input)
        .expect("pipeline parse should succeed");
    assert_eq!(parsed_functions.len(), 1, "expected one parsed function");

    let function = parsed_functions[0];
    let rendered = function.sprint(&parsed_pipeline);

    let mut reparsed_pipeline: Pipeline<StageInfo<BitwiseFunctionLanguage>> = Pipeline::new();
    let reparsed_functions = reparsed_pipeline
        .parse(&rendered)
        .expect("rendered pipeline should reparse");
    assert_eq!(
        reparsed_functions.len(),
        1,
        "expected one reparsed function"
    );

    let rendered_again = reparsed_functions[0].sprint(&reparsed_pipeline);
    assert_eq!(rendered.trim_end(), rendered_again.trim_end());
}
