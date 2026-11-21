use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_function::Return;
use kirin_test_utils::roundtrip;

/// Language with inlined Constant variant (inlined to test the format-derived
/// parser path for generic value types with `#[kirin(into)]` + `#[kirin(type = ...)]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum ConstantLanguage {
    #[chumsky(format = "fn {:name}{sig} {body}")]
    Function {
        body: CFG,
        sig: Signature<ArithType>,
    },
    #[kirin(constant, pure)]
    #[chumsky(format = "$constant {value} -> {result:type}")]
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

specialize @test fn @main() -> i64 cfg {
  ^entry() {
    %x = constant 42 -> i64;
    ret %x;
  }
}
"#;
    roundtrip::assert_program_roundtrip::<ConstantLanguage>(input);
}

#[test]
fn test_constant_f64() {
    let input = r#"
stage @test fn @main() -> f64;

specialize @test fn @main() -> f64 cfg {
  ^entry() {
    %x = constant 3.14 -> f64;
    ret %x;
  }
}
"#;
    roundtrip::assert_program_roundtrip::<ConstantLanguage>(input);
}

#[test]
fn test_constant_with_arithmetic() {
    let input = r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 cfg {
  ^entry() {
    %a = constant 10 -> i64;
    %b = constant 20 -> i64;
    %sum = add %a, %b -> i64;
    ret %sum;
  }
}
"#;
    roundtrip::assert_program_roundtrip::<ConstantLanguage>(input);
}

#[test]
fn test_sig_multiple_and_no_returns() {
    let input = r#"
stage @test fn @pair(i64, f64) -> (i64, f64);
stage @test fn @sink(i64) -> None;

specialize @test fn @pair(i64, f64) -> (i64, f64) cfg {
  ^entry(%a: i64, %b: f64) {
    ret %a, %b;
  }
}

specialize @test fn @sink(i64) -> None cfg {
  ^entry(%a: i64) {
    ret;
  }
}
"#;
    roundtrip::assert_program_roundtrip::<ConstantLanguage>(input);
}
