use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Lexical, Return};
use kirin_scf::StructuredControlFlow;
use kirin_test_utils::roundtrip;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum ComposedSourceLanguage {
    #[wraps]
    Function(Lexical<ArithType>),
    #[wraps]
    Scf(StructuredControlFlow<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum WrappedConstantLanguage {
    #[wraps]
    Function(FunctionBody<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum WrappedConstantStatementLanguage {
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

#[test]
fn test_composed_source_language_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    %doubled = add %x, %x -> i64;
    if %cond then ^then() {
      yield %doubled;
    } else ^else() {
      yield %x;
    };
    %captured = constant 41 -> i64;
    %closure = lambda @adder captures(%captured) {
      ^bb0(%arg: i64) {
        %sum = add %captured, %arg -> i64;
        ret %sum;
      }
    } -> i64;
    %result = call @adder(%x) -> i64;
    ret %result;
  }
}
"#;

    roundtrip::assert_pipeline_roundtrip::<ComposedSourceLanguage>(input);
}

#[test]
fn test_wrapped_constant_roundtrip() {
    let input = r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %lhs = constant 20 -> i64;
    %rhs = constant 22 -> i64;
    %sum = add %lhs, %rhs -> i64;
    ret %sum;
  }
}
"#;

    roundtrip::assert_pipeline_roundtrip::<WrappedConstantLanguage>(input);
}

#[test]
fn test_wrapped_constant_statement_roundtrip() {
    roundtrip::assert_statement_roundtrip::<WrappedConstantStatementLanguage>(
        "%x = constant 42 -> i64",
        &[],
    );
}
