//! Tests for kirin-chumsky runtime API
//!
//! Note: Tests for parsers that require a full Dialect implementation are in
//! the integration tests in `kirin-chumsky-derive/tests/integration.rs`.
//! This file contains tests for simpler parsers that don't require the full
//! dialect infrastructure.

use crate::ast::*;
use crate::parsers::*;
use chumsky::prelude::*;

// === Helper macro to create parsers ===

macro_rules! test_parse {
    ($input:expr, $parser:expr) => {
        kirin_test_utils::parse_tokens!($input, $parser)
    };
}

// === Identifier Tests ===

#[test]
fn test_identifier_exact_match() {
    let result = test_parse!("add", identifier("add"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "add");
}

#[test]
fn test_identifier_no_match() {
    let result = test_parse!("sub", identifier("add"));
    assert!(result.is_err());
}

#[test]
fn test_any_identifier() {
    let result = test_parse!("foo", any_identifier());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "foo");
}

#[test]
fn test_any_identifier_underscore() {
    let result = test_parse!("_foo_bar", any_identifier());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "_foo_bar");
}

// === Symbol Tests ===

#[test]
fn test_symbol_parser() {
    let result = test_parse!("@main", symbol());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "main");
}

#[test]
fn test_symbol_no_match() {
    let result: Result<SymbolName<'_>, _> = test_parse!("main", symbol());
    assert!(result.is_err());
}

#[test]
fn test_symbol_with_underscore() {
    let result = test_parse!("@my_function", symbol());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "my_function");
}

// === SSA Name Tests ===

#[test]
fn test_ssa_name() {
    let result = test_parse!("%value", ssa_name());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "value");
}

#[test]
fn test_ssa_name_numeric() {
    let result = test_parse!("%0", ssa_name());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "0");
}

#[test]
fn test_ssa_name_with_underscore() {
    let result = test_parse!("%my_value", ssa_name());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "my_value");
}

// === Block Label Tests ===

#[test]
fn test_block_label() {
    let result = test_parse!("^bb0", block_label());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name.value, "bb0");
}

#[test]
fn test_block_label_with_name() {
    let result = test_parse!("^entry", block_label());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name.value, "entry");
}

#[test]
fn test_block_label_numeric() {
    let result = test_parse!("^0", block_label());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name.value, "0");
}

// === Literal Integer Tests ===

#[test]
fn test_literal_int() {
    let result = test_parse!(
        "42",
        literal_int(|s, span| s.parse::<i32>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, 42);
}

#[test]
fn test_literal_int_u64() {
    let result = test_parse!(
        "123",
        literal_int(|s, span| s.parse::<u64>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, 123u64);
}

#[test]
fn test_literal_int_large() {
    let result = test_parse!(
        "9999999999",
        literal_int(|s, span| s.parse::<u64>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, 9999999999u64);
}

// === Literal Float Tests ===

#[test]
fn test_literal_float() {
    let result = test_parse!(
        "3.14",
        literal_float(|s, span| s
            .parse::<f64>()
            .map_err(|_| Rich::custom(span, "bad float")))
    );
    assert!(result.is_ok());
    let f = result.unwrap().value;
    assert!((f - 3.14).abs() < 0.001);
}

#[test]
fn test_literal_float_no_leading_zero() {
    let result = test_parse!(
        "0.5",
        literal_float(|s, span| s
            .parse::<f64>()
            .map_err(|_| Rich::custom(span, "bad float")))
    );
    assert!(result.is_ok());
    let f = result.unwrap().value;
    assert!((f - 0.5).abs() < 0.001);
}

// === Nameof SSA Tests ===

#[test]
fn test_nameof_ssa() {
    let result = test_parse!("%myvalue", nameof_ssa());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "myvalue");
}

#[test]
fn test_nameof_ssa_numeric() {
    let result = test_parse!("%123", nameof_ssa());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "123");
}

// === SSA Value Tests ===

#[test]
fn test_ssa_value_without_type() {
    let result = test_parse!("%x", ssa_value::<_, i32>());
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.name.value, "x");
    assert!(v.ty.is_none());
}

#[test]
fn test_ssa_value_with_type() {
    let result = test_parse!("%x : 42", ssa_value::<_, i32>());
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.name.value, "x");
    assert_eq!(v.ty, Some(42));
}

// === Result Value Tests ===

#[test]
fn test_result_value_without_type() {
    let result = test_parse!("%res", result_value::<_, i32>());
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.name.value, "res");
    assert!(v.ty.is_none());
}

#[test]
fn test_result_value_with_type() {
    let result = test_parse!("%res : 10", result_value::<_, i32>());
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.name.value, "res");
    assert_eq!(v.ty, Some(10));
}

// === Typeof SSA Tests ===

#[test]
fn test_typeof_ssa() {
    let result = test_parse!("42", typeof_ssa::<_, i32>());
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.ty, 42);
}

// === Block Argument Tests ===

#[test]
fn test_block_argument() {
    let result = test_parse!("%arg : 32", block_argument::<_, i32>());
    assert!(result.is_ok());
    let ba = result.unwrap();
    assert_eq!(ba.value.name.value, "arg");
    assert_eq!(ba.value.ty.value, 32);
}

// === Block Argument List Tests ===

#[test]
fn test_block_argument_list_single() {
    let result = test_parse!("( %a : 1 )", block_argument_list::<_, i32>());
    assert!(result.is_ok());
    let args = result.unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].value.name.value, "a");
    assert_eq!(args[0].value.ty.value, 1);
}

#[test]
fn test_block_argument_list_multiple() {
    let result = test_parse!("( %a : 1 , %b : 2 )", block_argument_list::<_, i32>());
    assert!(result.is_ok());
    let args = result.unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].value.name.value, "a");
    assert_eq!(args[1].value.name.value, "b");
}

#[test]
fn test_block_argument_list_empty() {
    let result = test_parse!("()", block_argument_list::<_, i32>());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_block_argument_list_trailing_comma() {
    let result = test_parse!("( %a : 1 , )", block_argument_list::<_, i32>());
    assert!(result.is_ok());
    let args = result.unwrap();
    assert_eq!(args.len(), 1);
}

// === Block Header Tests ===

#[test]
fn test_block_header_with_args() {
    let result = test_parse!("^bb0 ( %x : 1 )", block_header::<_, i32>());
    assert!(result.is_ok());
    let header = result.unwrap();
    assert_eq!(header.value.label.name.value, "bb0");
    assert_eq!(header.value.arguments.len(), 1);
    assert_eq!(header.value.arguments[0].value.name.value, "x");
}

#[test]
fn test_block_header_no_args() {
    let result = test_parse!("^entry", block_header::<_, i32>());
    assert!(result.is_ok());
    let header = result.unwrap();
    assert_eq!(header.value.label.name.value, "entry");
    assert!(header.value.arguments.is_empty());
}

#[test]
fn test_block_header_empty_parens() {
    let result = test_parse!("^bb0 ()", block_header::<_, i32>());
    assert!(result.is_ok());
    let header = result.unwrap();
    assert_eq!(header.value.label.name.value, "bb0");
    assert!(header.value.arguments.is_empty());
}

// === Function Type Tests ===

#[test]
fn test_function_type_single_arg_single_ret() {
    let result = test_parse!("( 1 ) -> 2", function_type::<_, i32>());
    assert!(result.is_ok());
    let ft = result.unwrap();
    assert_eq!(ft.value.input_types.len(), 1);
    assert_eq!(ft.value.input_types[0].value, 1);
    assert_eq!(ft.value.output_types.len(), 1);
    assert_eq!(ft.value.output_types[0].value, 2);
}

#[test]
fn test_function_type_no_args() {
    let result = test_parse!("-> 5", function_type::<_, i32>());
    assert!(result.is_ok());
    let ft = result.unwrap();
    assert!(ft.value.input_types.is_empty());
    assert_eq!(ft.value.output_types.len(), 1);
    assert_eq!(ft.value.output_types[0].value, 5);
}

#[test]
fn test_function_type_multi_return() {
    let result = test_parse!("( 1 ) -> ( 2 , 3 )", function_type::<_, i32>());
    assert!(result.is_ok());
    let ft = result.unwrap();
    assert_eq!(ft.value.input_types.len(), 1);
    assert_eq!(ft.value.output_types.len(), 2);
    assert_eq!(ft.value.output_types[0].value, 2);
    assert_eq!(ft.value.output_types[1].value, 3);
}

#[test]
fn test_function_type_multi_args() {
    let result = test_parse!("( 1 , 2 , 3 ) -> 4", function_type::<_, i32>());
    assert!(result.is_ok());
    let ft = result.unwrap();
    assert_eq!(ft.value.input_types.len(), 3);
    assert_eq!(ft.value.output_types.len(), 1);
}

// === EmitContext Tests ===

/// Minimal type for the test dialect.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct TestType;

impl std::fmt::Display for TestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "test")
    }
}

/// Minimal dialect for EmitContext tests (avoids circular dependency on kirin-test-languages).
#[derive(Clone, Debug, PartialEq, Eq, Hash, kirin_derive_ir::Dialect)]
#[kirin(crate = kirin_ir, type = TestType)]
enum TestDialect {
    Noop,
}

#[test]
fn test_emit_context_ssa_lookup() {
    let mut stage: kirin_ir::StageInfo<TestDialect> = kirin_ir::StageInfo::default();
    let ssa = kirin_ir::SSAValue::from(stage.block_argument(0));

    let mut ctx = crate::traits::EmitContext::new(&mut stage);

    // Initially empty
    assert!(ctx.lookup_ssa("x").is_none());

    // Register and lookup
    ctx.register_ssa("x".to_string(), ssa);
    assert_eq!(ctx.lookup_ssa("x"), Some(ssa));

    // Missing still returns None
    assert!(ctx.lookup_ssa("y").is_none());
}

#[test]
fn test_emit_context_block_lookup() {
    let mut stage: kirin_ir::StageInfo<TestDialect> = kirin_ir::StageInfo::default();
    let block = stage.block().new();

    let mut ctx = crate::traits::EmitContext::new(&mut stage);

    // Initially empty
    assert!(ctx.lookup_block("bb0").is_none());

    // Register and lookup
    ctx.register_block("bb0".to_string(), block);
    assert_eq!(ctx.lookup_block("bb0"), Some(block));
}

#[test]
fn test_emit_context_ssa_overwrite() {
    let mut stage: kirin_ir::StageInfo<TestDialect> = kirin_ir::StageInfo::default();
    let ssa1 = kirin_ir::SSAValue::from(stage.block_argument(0));
    let ssa2 = kirin_ir::SSAValue::from(stage.block_argument(1));

    let mut ctx = crate::traits::EmitContext::new(&mut stage);
    ctx.register_ssa("x".to_string(), ssa1);
    ctx.register_ssa("x".to_string(), ssa2);
    assert_eq!(ctx.lookup_ssa("x"), Some(ssa2));
}
