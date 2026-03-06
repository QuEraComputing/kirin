use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_function::Return;
use kirin_test_utils::roundtrip;

// SCF ops have Block fields, can't use #[wraps] due to E0275.
// Inline the SCF variants directly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum ScfLanguage {
    #[chumsky(format = "{body}")]
    Function { body: Region },
    #[chumsky(format = "{.if} {condition} then {then_body} else {else_body}")]
    If {
        condition: SSAValue,
        then_body: Block,
        else_body: Block,
    },
    #[kirin(terminator)]
    #[chumsky(format = "{.yield} {value}")]
    Yield { value: SSAValue },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

#[test]
fn test_if_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    %doubled = add %x, %x -> i64;
    if %cond then ^then() {
      %r = add %doubled, %doubled -> i64;
    } else ^else() {
      %r2 = sub %doubled, %doubled -> i64;
    };
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}

#[test]
fn test_yield_in_if_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    if %cond then ^then() {
      yield %x;
    } else ^else() {
      yield %x;
    };
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}
