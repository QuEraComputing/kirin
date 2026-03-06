//! Integration tests for namespace prefix roundtripping.

use kirin_arith::ArithType;
use kirin_test_languages::{ArithFunctionLanguage, NamespacedLanguage};
use kirin_test_utils::roundtrip;

// NamespacedLanguage tests (arith.add, cf.br, func.ret prefixes)

#[test]
fn test_namespace_pipeline_roundtrip() {
    let input = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %sum = arith.add %a, %b -> i64;
    %diff = arith.sub %sum, %b -> i64;
    %neg = arith.neg %diff -> i64;
    func.ret %neg;
  }
}
"#;

    roundtrip::assert_pipeline_roundtrip::<NamespacedLanguage>(input);
}

#[test]
fn test_namespace_statement_roundtrip_add() {
    roundtrip::assert_statement_roundtrip::<NamespacedLanguage>(
        "%sum = arith.add %a, %b -> i64",
        &[("a", ArithType::I64), ("b", ArithType::I64)],
    );
}

#[test]
fn test_namespace_statement_roundtrip_sub() {
    roundtrip::assert_statement_roundtrip::<NamespacedLanguage>(
        "%diff = arith.sub %a, %b -> i32",
        &[("a", ArithType::I32), ("b", ArithType::I32)],
    );
}

#[test]
fn test_namespace_statement_roundtrip_neg() {
    roundtrip::assert_statement_roundtrip::<NamespacedLanguage>(
        "%neg = arith.neg %a -> f64",
        &[("a", ArithType::F64)],
    );
}

#[test]
fn test_namespace_statement_roundtrip_ret() {
    roundtrip::assert_statement_roundtrip::<NamespacedLanguage>(
        "func.ret %v",
        &[("v", ArithType::I64)],
    );
}

// BareLanguage tests -> use ArithFunctionLanguage (structurally identical)

#[test]
fn test_bare_statement_roundtrip_add() {
    roundtrip::assert_statement_roundtrip::<ArithFunctionLanguage>(
        "%sum = add %a, %b -> i64",
        &[("a", ArithType::I64), ("b", ArithType::I64)],
    );
}

#[test]
fn test_bare_statement_roundtrip_ret() {
    roundtrip::assert_statement_roundtrip::<ArithFunctionLanguage>(
        "ret %v",
        &[("v", ArithType::I64)],
    );
}

#[test]
fn test_bare_pipeline_roundtrip() {
    let input = r#"
stage @test fn @compose(i64, i64) -> i64;

specialize @test fn @compose(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %sum = add %a, %b -> i64;
    %diff = sub %sum, %b -> i64;
    ret %diff;
  }
}
"#;

    roundtrip::assert_pipeline_roundtrip::<ArithFunctionLanguage>(input);
}
