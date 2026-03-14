use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_function::Return;
use kirin_test_utils::roundtrip;

/// Language with inlined Constant variant (inlined to test the format-derived
/// parser path for generic value types with `#[kirin(into)]` + `#[kirin(type = ...)]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum ConstantLanguage {
    #[chumsky(format = "{body}")]
    Function { body: Region },
    #[kirin(constant, pure)]
    #[chumsky(format = "{result:name} = {.constant} {value} -> {result:type}")]
    Constant {
        #[kirin(into)]
        value: ArithValue,
        #[kirin(type = value.type_of())]
        result: ResultValue,
    },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

#[test]
fn test_constant_i64() {
    let input = r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %x = constant 42 -> i64;
    ret %x;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ConstantLanguage>(input);
}

#[test]
fn test_constant_f64() {
    let input = r#"
stage @test fn @main() -> f64;

specialize @test fn @main() -> f64 {
  ^entry() {
    %x = constant 3.14 -> f64;
    ret %x;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ConstantLanguage>(input);
}

#[test]
fn test_constant_with_arithmetic() {
    let input = r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 10 -> i64;
    %b = constant 20 -> i64;
    %sum = add %a, %b -> i64;
    ret %sum;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ConstantLanguage>(input);
}
