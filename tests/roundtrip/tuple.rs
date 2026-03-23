use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_function::Return;
use kirin_test_utils::roundtrip;
use kirin_tuple::Tuple;

/// Language with Tuple + Function + Return for pipeline roundtrip tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum TupleLanguage {
    #[chumsky(format = "fn {:name}{sig} {body}")]
    Function {
        body: Region,
        sig: Signature<ArithType>,
    },
    #[wraps]
    Tuple(Tuple<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

// --- Statement-level roundtrip tests ---

#[test]
fn test_new_tuple_roundtrip() {
    roundtrip::assert_statement_roundtrip::<TupleLanguage>(
        "%t = new_tuple(%x, %y) -> i32",
        &[("x", ArithType::I32), ("y", ArithType::I32)],
    );
}

#[test]
fn test_new_tuple_single_arg_roundtrip() {
    roundtrip::assert_statement_roundtrip::<TupleLanguage>(
        "%t = new_tuple(%x) -> i64",
        &[("x", ArithType::I64)],
    );
}

#[test]
fn test_unpack_roundtrip() {
    roundtrip::assert_statement_roundtrip::<TupleLanguage>(
        "%a, %b = unpack %t -> i32, i64",
        &[("t", ArithType::I32)],
    );
}

#[test]
fn test_unpack_single_result_roundtrip() {
    roundtrip::assert_statement_roundtrip::<TupleLanguage>(
        "%a = unpack %t -> i32",
        &[("t", ArithType::I32)],
    );
}

// --- Pipeline-level roundtrip tests ---

#[test]
fn test_new_tuple_pipeline_roundtrip() {
    let input = r#"
stage @test fn @main(i32, i32) -> i32;

specialize @test fn @main(i32, i32) -> i32 {
  ^entry(%x: i32, %y: i32) {
    %t = new_tuple(%x, %y) -> i32;
    ret %t;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<TupleLanguage>(input);
}

#[test]
fn test_unpack_pipeline_roundtrip() {
    let input = r#"
stage @test fn @main(i32) -> i32;

specialize @test fn @main(i32) -> i32 {
  ^entry(%t: i32) {
    %a, %b = unpack %t -> i32, i32;
    ret %a;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<TupleLanguage>(input);
}

#[test]
fn test_new_tuple_then_unpack_pipeline_roundtrip() {
    let input = r#"
stage @test fn @main(i32, i32) -> i32;

specialize @test fn @main(i32, i32) -> i32 {
  ^entry(%x: i32, %y: i32) {
    %t = new_tuple(%x, %y) -> i32;
    %a, %b = unpack %t -> i32, i32;
    ret %a;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<TupleLanguage>(input);
}
