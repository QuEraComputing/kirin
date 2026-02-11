use kirin::ir::{Dialect, Region};
use kirin::parsers::{HasParser, PrettyPrint};
use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;
use kirin_test_utils::roundtrip;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum ArithmeticFunctionLanguage {
    #[chumsky(format = "{body}")]
    Function { body: Region },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
}

#[test]
fn test_arithmetic_function_roundtrip_print_parse_print() {
    let input = r#"
stage @arith fn @compose(i64, i64, f64, f64) -> i64;
specialize @arith fn @compose(i64, i64, f64, f64) -> i64 {
  ^entry(%a: i64, %b: i64, %x: f64, %y: f64) {
    %sum = add %a, %b -> i64;
    %diff = sub %sum, %b -> i64;
    %prod = mul %diff, %a -> i64;
    %quot = div %prod, %a -> i64;
    %rem = rem %quot, %b -> i64;
    %neg = neg %rem -> i64;
    %fsum = add %x, %y -> f64;
    %fscaled = mul %fsum, %x -> f64;
    %fneg = neg %fscaled -> f64;
    ret %neg;
  }
}
"#;

    roundtrip::assert_pipeline_roundtrip::<ArithmeticFunctionLanguage>(input);
}
