use kirin::ir::{Dialect, Region};
use kirin::parsers::{HasParser, PrettyPrint};
use kirin_arith::ArithType;
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_test_utils::roundtrip;

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

    roundtrip::assert_pipeline_roundtrip::<BitwiseFunctionLanguage>(input);
}
