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
    %r = if %cond then ^then() {
      %r2 = add %doubled, %doubled -> i64;
    } else ^else() {
      %r3 = sub %doubled, %doubled -> i64;
    } -> i64;
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
    %result = if %cond then ^then() {
      yield %x;
    } else ^else() {
      yield %x;
    } -> i64;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}

#[test]
fn test_for_with_iter_args_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64) {
    %init = add %lo, %lo -> i64;
    %sum = for %lo in %lo..%hi step %s iter_args(%init) do ^body(%i: i64, %acc: i64) {
      %next = add %acc, %i -> i64;
      yield %next;
    } -> i64;
    ret %sum;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}

#[test]
fn test_for_no_iter_args_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64) {
    %r = for %lo in %lo..%hi step %s iter_args() do ^body(%i: i64) {
      yield %i;
    } -> i64;
    ret %r;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}

#[test]
fn test_void_if_roundtrip() {
    let input = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%cond: i64) {
    if %cond then ^then() {
      yield;
    } else ^else() {
      yield;
    };
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}

#[test]
fn test_multi_result_if_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    %a, %b = if %cond then ^then() {
      yield %x, %x;
    } else ^else() {
      yield %x, %x;
    } -> i64, i64;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}

#[test]
fn test_multi_accumulator_for_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64, %init2: i64) {
    %init1 = add %lo, %lo -> i64;
    %r1, %r2 = for %lo in %lo..%hi step %s iter_args(%init1, %init2) do ^body(%i: i64, %acc1: i64, %acc2: i64) {
      %next1 = add %acc1, %i -> i64;
      %next2 = add %acc2, %i -> i64;
      yield %next1, %next2;
    } -> i64, i64;
    ret %r1;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<ScfLanguage>(input);
}
