use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_function::Return;
use kirin_scf::StructuredControlFlow;
use kirin_test_utils::roundtrip;

// SCF ops use #[wraps] delegation — Block-containing types work with #[wraps].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum ScfLanguage {
    #[chumsky(format = "fn {:name}{sig} {body}")]
    Function {
        body: Region,
        sig: Signature<ArithType>,
    },
    #[wraps]
    Scf(StructuredControlFlow<ArithType>),
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
