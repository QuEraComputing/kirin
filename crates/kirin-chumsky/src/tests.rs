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

// === parse_ast Tests ===

#[test]
fn test_parse_ast_empty_input() {
    let result = crate::traits::parse_ast::<i32>("");
    assert!(result.is_err());
}

#[test]
fn test_parse_ast_valid_int() {
    let result = crate::traits::parse_ast::<i32>("42");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_parse_ast_invalid_input() {
    let result = crate::traits::parse_ast::<i32>("not_a_number");
    assert!(result.is_err());
}

#[test]
fn test_parse_ast_multiple_errors() {
    // Completely invalid token sequence should produce errors
    let result = crate::traits::parse_ast::<i32>("@#$");
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

// === ParseError Display Tests ===

#[test]
fn test_parse_error_display() {
    let err = crate::traits::ParseError {
        message: "unexpected token".to_string(),
        span: SimpleSpan::from(5..10),
    };
    let display = format!("{}", err);
    assert_eq!(display, "error at 5..10: unexpected token");
}

#[test]
fn test_parse_error_is_std_error() {
    let err = crate::traits::ParseError {
        message: "test".to_string(),
        span: SimpleSpan::from(0..1),
    };
    // Verify it implements std::error::Error
    let _: &dyn std::error::Error = &err;
}

// === FunctionParseError Display Tests ===

#[test]
fn test_function_parse_error_display_with_span() {
    let err = crate::FunctionParseError::new(
        crate::FunctionParseErrorKind::InvalidHeader,
        Some(SimpleSpan::from(3..7)),
        "missing semicolon",
    );
    let display = format!("{}", err);
    assert_eq!(
        display,
        "invalid function header at 3..7: missing semicolon"
    );
}

#[test]
fn test_function_parse_error_display_without_span() {
    let err = crate::FunctionParseError::new(
        crate::FunctionParseErrorKind::UnknownStage,
        None,
        "stage not found",
    );
    let display = format!("{}", err);
    assert_eq!(display, "unknown stage: stage not found");
}

#[test]
fn test_function_parse_error_kind_display() {
    assert_eq!(
        format!("{}", crate::FunctionParseErrorKind::InvalidHeader),
        "invalid function header"
    );
    assert_eq!(
        format!("{}", crate::FunctionParseErrorKind::UnknownStage),
        "unknown stage"
    );
    assert_eq!(
        format!(
            "{}",
            crate::FunctionParseErrorKind::InconsistentFunctionName
        ),
        "inconsistent function name"
    );
    assert_eq!(
        format!("{}", crate::FunctionParseErrorKind::MissingStageDeclaration),
        "missing stage declaration"
    );
    assert_eq!(
        format!("{}", crate::FunctionParseErrorKind::BodyParseFailed),
        "function body parse failed"
    );
    assert_eq!(
        format!("{}", crate::FunctionParseErrorKind::EmitFailed),
        "IR emission failed"
    );
}

#[test]
fn test_function_parse_error_source() {
    use std::error::Error;

    // Without source
    let err =
        crate::FunctionParseError::new(crate::FunctionParseErrorKind::InvalidHeader, None, "test");
    assert!(err.source().is_none());

    // With source
    let source_err = std::io::Error::new(std::io::ErrorKind::Other, "inner");
    let err = crate::FunctionParseError::new(
        crate::FunctionParseErrorKind::BodyParseFailed,
        None,
        "outer",
    )
    .with_source(source_err);
    assert!(err.source().is_some());
}

// === Error path tests: missing prefix ===

#[test]
fn test_ssa_name_missing_percent() {
    let result: Result<Spanned<&str>, _> = test_parse!("value", ssa_name());
    assert!(result.is_err(), "SSA name without % prefix should fail");
}

#[test]
fn test_block_label_missing_caret() {
    let result: Result<BlockLabel<'_>, _> = test_parse!("block0", block_label());
    assert!(result.is_err(), "block label without ^ prefix should fail");
}

#[test]
fn test_symbol_missing_at_with_identifier() {
    // "func_name" is just an identifier, not a symbol
    let result: Result<SymbolName<'_>, _> = test_parse!("func_name", symbol());
    assert!(result.is_err(), "symbol without @ prefix should fail");
}

#[test]
fn test_ssa_name_with_at_prefix() {
    // @ is for symbols, not SSA values
    let result: Result<Spanned<&str>, _> = test_parse!("@value", ssa_name());
    assert!(result.is_err(), "SSA name with @ prefix should fail");
}

#[test]
fn test_block_label_with_percent_prefix() {
    // % is for SSA values, not block labels
    let result: Result<BlockLabel<'_>, _> = test_parse!("%entry", block_label());
    assert!(result.is_err(), "block label with % prefix should fail");
}

#[test]
fn test_symbol_with_percent_prefix() {
    // % is for SSA values, not symbols
    let result: Result<SymbolName<'_>, _> = test_parse!("%main", symbol());
    assert!(result.is_err(), "symbol with % prefix should fail");
}

// === Empty input error tests ===

#[test]
fn test_empty_input_fails_for_ssa_name() {
    let result: Result<Spanned<&str>, _> = test_parse!("", ssa_name());
    assert!(result.is_err(), "empty input should fail for ssa_name");
}

#[test]
fn test_empty_input_fails_for_identifier() {
    let result: Result<Spanned<&str>, _> = test_parse!("", any_identifier());
    assert!(
        result.is_err(),
        "empty input should fail for any_identifier"
    );
}

#[test]
fn test_empty_input_fails_for_symbol() {
    let result: Result<SymbolName<'_>, _> = test_parse!("", symbol());
    assert!(result.is_err(), "empty input should fail for symbol");
}

#[test]
fn test_empty_input_fails_for_block_label() {
    let result: Result<BlockLabel<'_>, _> = test_parse!("", block_label());
    assert!(result.is_err(), "empty input should fail for block_label");
}

#[test]
fn test_empty_input_fails_for_literal_int() {
    let result: Result<Spanned<i32>, _> = test_parse!(
        "",
        literal_int(|s, span| s.parse::<i32>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_err(), "empty input should fail for literal_int");
}

#[test]
fn test_empty_input_fails_for_literal_float() {
    let result: Result<Spanned<f64>, _> = test_parse!(
        "",
        literal_float(|s, span| s
            .parse::<f64>()
            .map_err(|_| Rich::custom(span, "bad float")))
    );
    assert!(result.is_err(), "empty input should fail for literal_float");
}

// === Integer edge cases ===

#[test]
fn test_literal_int_zero() {
    let result = test_parse!(
        "0",
        literal_int(|s, span| s.parse::<i32>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, 0);
}

#[test]
fn test_literal_int_negative() {
    let result = test_parse!(
        "-42",
        literal_int(|s, span| s.parse::<i32>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, -42);
}

#[test]
fn test_literal_int_overflow_i32() {
    // 2^31 overflows i32
    let result: Result<Spanned<i32>, _> = test_parse!(
        "2147483648",
        literal_int(|s, span| s.parse::<i32>().map_err(|_| Rich::custom(span, "overflow")))
    );
    assert!(result.is_err(), "i32 overflow should produce an error");
}

#[test]
fn test_literal_int_max_i32() {
    let result = test_parse!(
        "2147483647",
        literal_int(|s, span| s.parse::<i32>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, i32::MAX);
}

// === Float edge cases ===

#[test]
fn test_literal_float_negative() {
    let result = test_parse!(
        "-1.5",
        literal_float(|s, span| s
            .parse::<f64>()
            .map_err(|_| Rich::custom(span, "bad float")))
    );
    assert!(result.is_ok());
    assert!((result.unwrap().value - (-1.5)).abs() < 0.001);
}

#[test]
fn test_literal_float_scientific_notation() {
    let result = test_parse!(
        "1.0e3",
        literal_float(|s, span| s
            .parse::<f64>()
            .map_err(|_| Rich::custom(span, "bad float")))
    );
    assert!(result.is_ok());
    assert!((result.unwrap().value - 1000.0).abs() < 0.001);
}

#[test]
fn test_literal_float_negative_exponent() {
    let result = test_parse!(
        "2.5e-2",
        literal_float(|s, span| s
            .parse::<f64>()
            .map_err(|_| Rich::custom(span, "bad float")))
    );
    assert!(result.is_ok());
    assert!((result.unwrap().value - 0.025).abs() < 0.001);
}

#[test]
fn test_integer_is_not_float() {
    // An integer literal should not parse as a float
    let result: Result<Spanned<f64>, _> = test_parse!(
        "42",
        literal_float(|s, span| s
            .parse::<f64>()
            .map_err(|_| Rich::custom(span, "bad float")))
    );
    assert!(result.is_err(), "plain integer should not parse as float");
}

#[test]
fn test_float_is_not_integer() {
    // A float literal should not parse as an integer
    let result: Result<Spanned<i32>, _> = test_parse!(
        "3.14",
        literal_int(|s, span| s.parse::<i32>().map_err(|_| Rich::custom(span, "bad int")))
    );
    assert!(result.is_err(), "float should not parse as integer");
}

// === Identifier mismatch tests ===

#[test]
fn test_identifier_wrong_keyword() {
    let result = test_parse!("mul", identifier("add"));
    assert!(result.is_err(), "identifier should reject wrong keyword");
}

#[test]
fn test_identifier_case_sensitive() {
    let result = test_parse!("Add", identifier("add"));
    assert!(
        result.is_err(),
        "identifier matching should be case-sensitive"
    );
}

#[test]
fn test_identifier_prefix_is_not_match() {
    // "adder" should not match "add"
    let result = test_parse!("adder", identifier("add"));
    assert!(
        result.is_err(),
        "identifier should require exact match, not prefix"
    );
}

// === Block argument error tests ===

#[test]
fn test_block_argument_missing_colon() {
    // Block argument requires %name : type
    let result: Result<Spanned<BlockArgument<'_, i32>>, _> =
        test_parse!("%arg 32", block_argument::<_, i32>());
    assert!(result.is_err(), "block argument without colon should fail");
}

#[test]
fn test_block_argument_missing_type() {
    // Block argument requires a type after the colon
    let result: Result<Spanned<BlockArgument<'_, i32>>, _> =
        test_parse!("%arg :", block_argument::<_, i32>());
    assert!(result.is_err(), "block argument without type should fail");
}

#[test]
fn test_block_argument_list_missing_closing_paren() {
    let result: Result<Vec<Spanned<BlockArgument<'_, i32>>>, _> =
        test_parse!("( %a : 1", block_argument_list::<_, i32>());
    assert!(
        result.is_err(),
        "block argument list without closing paren should fail"
    );
}

// === Function type error tests ===

#[test]
fn test_function_type_missing_arrow() {
    // "(1)" alone (no arrow) parses as empty output types, which is valid
    // but "(1) 2" should fail since there's a trailing unconsumed token
    let result = test_parse!("( 1 ) -> ->", function_type::<_, i32>());
    assert!(result.is_err(), "double arrow in function type should fail");
}

// === SSA value with type annotation edge cases ===

#[test]
fn test_ssa_value_colon_but_missing_type() {
    // %x : (nothing) should fail since there's a colon but no valid type
    let result: Result<SSAValue<'_, i32>, _> = test_parse!("%x :", ssa_value::<_, i32>());
    assert!(
        result.is_err(),
        "SSA value with colon but missing type should fail"
    );
}

// === Nameof SSA edge cases ===

#[test]
fn test_nameof_ssa_not_symbol() {
    let result: Result<NameofSSAValue<'_>, _> = test_parse!("@sym", nameof_ssa());
    assert!(result.is_err(), "nameof_ssa should reject @ prefixed names");
}

#[test]
fn test_nameof_ssa_not_block() {
    let result: Result<NameofSSAValue<'_>, _> = test_parse!("^bb0", nameof_ssa());
    assert!(result.is_err(), "nameof_ssa should reject ^ prefixed names");
}

// === parse_ast edge cases ===

#[test]
fn test_parse_ast_whitespace_only() {
    let result = crate::traits::parse_ast::<i32>("   ");
    assert!(result.is_err(), "whitespace-only input should fail");
}

#[test]
fn test_parse_ast_trailing_tokens() {
    // "42 99" has a trailing token; parse_ast should reject it
    let result = crate::traits::parse_ast::<i32>("42 99");
    assert!(
        result.is_err(),
        "trailing tokens after valid parse should fail"
    );
}

// === EmitError Display Tests ===

#[test]
fn test_emit_error_undefined_ssa_display() {
    let err = crate::traits::EmitError::UndefinedSSA("x".to_string());
    assert_eq!(format!("{err}"), "undefined SSA value: %x");
}

#[test]
fn test_emit_error_undefined_block_display() {
    let err = crate::traits::EmitError::UndefinedBlock("bb0".to_string());
    assert_eq!(format!("{err}"), "undefined block: ^bb0");
}

#[test]
fn test_emit_error_custom_display() {
    let err = crate::traits::EmitError::Custom("something went wrong".to_string());
    assert_eq!(format!("{err}"), "something went wrong");
}

#[test]
fn test_emit_error_is_std_error() {
    let err = crate::traits::EmitError::UndefinedSSA("x".to_string());
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_emit_error_equality() {
    let a = crate::traits::EmitError::UndefinedSSA("x".to_string());
    let b = crate::traits::EmitError::UndefinedSSA("x".to_string());
    let c = crate::traits::EmitError::UndefinedSSA("y".to_string());
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// === Spanned utility tests ===

#[test]
fn test_spanned_new() {
    let s = crate::ast::Spanned::new(42, SimpleSpan::from(0..5));
    assert_eq!(s.value, 42);
    assert_eq!(s.span, SimpleSpan::from(0..5));
}

#[test]
fn test_spanned_map() {
    let s = crate::ast::Spanned::new(42, SimpleSpan::from(0..5));
    let mapped = s.map(|v| v * 2);
    assert_eq!(mapped.value, 84);
    assert_eq!(mapped.span, SimpleSpan::from(0..5));
}

#[test]
fn test_spanned_display() {
    let s = crate::ast::Spanned::new(42, SimpleSpan::from(0..5));
    assert_eq!(format!("{s}"), "42");
}

#[test]
fn test_spanned_equality_ignores_span() {
    let a = crate::ast::Spanned::new(42, SimpleSpan::from(0..5));
    let b = crate::ast::Spanned::new(42, SimpleSpan::from(10..20));
    assert_eq!(a, b, "Spanned equality should ignore span");
}

#[test]
fn test_spanned_inequality() {
    let a = crate::ast::Spanned::new(42, SimpleSpan::from(0..5));
    let b = crate::ast::Spanned::new(99, SimpleSpan::from(0..5));
    assert_ne!(a, b);
}

#[test]
fn test_spanned_copy_for_copy_types() {
    let s = crate::ast::Spanned::new(42i32, SimpleSpan::from(0..5));
    let s2 = s; // Copy
    assert_eq!(s.value, s2.value);
}

// === EmitContext: block overwrite ===

#[test]
fn test_emit_context_block_overwrite() {
    let mut stage: kirin_ir::StageInfo<TestDialect> = kirin_ir::StageInfo::default();
    let block1 = stage.block().new();
    let block2 = stage.block().new();

    let mut ctx = crate::traits::EmitContext::new(&mut stage);
    ctx.register_block("bb0".to_string(), block1);
    ctx.register_block("bb0".to_string(), block2);
    assert_eq!(ctx.lookup_block("bb0"), Some(block2));
}

// === Multiple SSA / block registrations ===

#[test]
fn test_emit_context_multiple_distinct_names() {
    let mut stage: kirin_ir::StageInfo<TestDialect> = kirin_ir::StageInfo::default();
    let ssa_a = kirin_ir::SSAValue::from(stage.block_argument(0));
    let ssa_b = kirin_ir::SSAValue::from(stage.block_argument(1));

    let mut ctx = crate::traits::EmitContext::new(&mut stage);
    ctx.register_ssa("a".to_string(), ssa_a);
    ctx.register_ssa("b".to_string(), ssa_b);
    assert_eq!(ctx.lookup_ssa("a"), Some(ssa_a));
    assert_eq!(ctx.lookup_ssa("b"), Some(ssa_b));
    assert_ne!(ssa_a, ssa_b);
}

// === Function type edge cases ===

#[test]
fn test_function_type_no_args_no_return() {
    // Just an arrow with no return type should give empty outputs
    let result = test_parse!("->", function_type::<_, i32>());
    assert!(result.is_ok());
    let ft = result.unwrap();
    assert!(ft.value.input_types.is_empty());
    assert!(ft.value.output_types.is_empty());
}

#[test]
fn test_function_type_empty_parens_input() {
    // "()" is parsed as empty type list not a valid i32, so no inputs
    // This depends on whether the type parser matches "()" — for i32 it shouldn't
    let result = test_parse!("( ) -> 1", function_type::<_, i32>());
    // Empty parens with no valid type inside should fail because there's no valid
    // i32 token between the parens, but separated_by allows empty
    assert!(result.is_ok());
    let ft = result.unwrap();
    assert!(ft.value.input_types.is_empty());
    assert_eq!(ft.value.output_types.len(), 1);
}

#[test]
fn test_function_type_trailing_comma_input() {
    // "(1,)" — trailing comma is not explicitly allowed for function_type input_types
    // separated_by default does not allow trailing comma
    let result = test_parse!("( 1 , ) -> 2", function_type::<_, i32>());
    assert!(
        result.is_err(),
        "function_type input_types should not allow trailing comma"
    );
}

#[test]
fn test_function_type_multi_return_parens() {
    let result = test_parse!("( 1 , 2 ) -> ( 3 , 4 )", function_type::<_, i32>());
    assert!(result.is_ok());
    let ft = result.unwrap();
    assert_eq!(ft.value.input_types.len(), 2);
    assert_eq!(ft.value.output_types.len(), 2);
    assert_eq!(ft.value.output_types[0].value, 3);
    assert_eq!(ft.value.output_types[1].value, 4);
}

// === Block header with multiple arguments ===

#[test]
fn test_block_header_multiple_args() {
    let result = test_parse!(
        "^bb0 ( %a : 1 , %b : 2 , %c : 3 )",
        block_header::<_, i32>()
    );
    assert!(result.is_ok());
    let header = result.unwrap();
    assert_eq!(header.value.arguments.len(), 3);
    assert_eq!(header.value.arguments[2].value.name.value, "c");
    assert_eq!(header.value.arguments[2].value.ty.value, 3);
}

#[test]
fn test_block_header_trailing_comma_in_args() {
    let result = test_parse!("^bb0 ( %a : 1 , )", block_header::<_, i32>());
    assert!(result.is_ok());
    let header = result.unwrap();
    assert_eq!(header.value.arguments.len(), 1);
}

// === SSA name edge cases ===

#[test]
fn test_ssa_name_mixed_alphanumeric() {
    let result = test_parse!("%arg0_val", ssa_name());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "arg0_val");
}

// === parse_ast error message content ===

#[test]
fn test_parse_error_contains_position_info() {
    let err = crate::traits::ParseError {
        message: "oops".to_string(),
        span: SimpleSpan::from(0..0),
    };
    let display = format!("{err}");
    assert!(
        display.contains("0..0"),
        "error display should contain span info"
    );
}

#[test]
fn test_parse_ast_errors_are_nonempty() {
    let result = crate::traits::parse_ast::<i32>("");
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
    // Each error should have a non-empty message
    for e in &errors {
        assert!(!e.message.is_empty(), "error message should not be empty");
    }
}

// === FunctionParseError edge cases ===

#[test]
fn test_function_parse_error_debug() {
    let err = crate::FunctionParseError::new(
        crate::FunctionParseErrorKind::EmitFailed,
        Some(SimpleSpan::from(10..20)),
        "emit error detail",
    );
    let debug = format!("{err:?}");
    assert!(debug.contains("EmitFailed"));
    assert!(debug.contains("emit error detail"));
}

#[test]
fn test_function_parse_error_kind_copy() {
    let kind = crate::FunctionParseErrorKind::InvalidHeader;
    let kind2 = kind; // Copy
    assert_eq!(kind, kind2);
}

#[test]
fn test_function_parse_error_all_kinds_display() {
    // Ensure all variants produce distinct non-empty Display output
    let kinds = [
        crate::FunctionParseErrorKind::InvalidHeader,
        crate::FunctionParseErrorKind::UnknownStage,
        crate::FunctionParseErrorKind::InconsistentFunctionName,
        crate::FunctionParseErrorKind::MissingStageDeclaration,
        crate::FunctionParseErrorKind::BodyParseFailed,
        crate::FunctionParseErrorKind::EmitFailed,
    ];
    let mut displays: Vec<String> = kinds.iter().map(|k| format!("{k}")).collect();
    for d in &displays {
        assert!(!d.is_empty());
    }
    displays.sort();
    displays.dedup();
    assert_eq!(
        displays.len(),
        kinds.len(),
        "all error kinds should have distinct display strings"
    );
}

// === parse_ast with special token inputs ===

#[test]
fn test_parse_ast_comment_only() {
    // Comments are filtered by the lexer; an input with only a comment is effectively empty
    let result = crate::traits::parse_ast::<i32>("/* nothing here */");
    assert!(result.is_err(), "comment-only input should fail");
}

#[test]
fn test_parse_ast_line_comment_only() {
    let result = crate::traits::parse_ast::<i32>("// nothing here\n");
    assert!(result.is_err(), "line-comment-only input should fail");
}

// === Lexer error token handling ===

#[test]
fn test_parse_ast_invalid_token() {
    // Backtick is not a valid token in the kirin lexer
    let result = crate::traits::parse_ast::<i32>("`");
    assert!(result.is_err());
}

#[test]
fn test_parse_ast_mixed_valid_invalid() {
    // Valid integer followed by an invalid token
    let result = crate::traits::parse_ast::<i32>("42 `");
    assert!(result.is_err());
}

// === Multiple result values and SSA values ===

#[test]
fn test_result_value_numeric_name() {
    let result = test_parse!("%0", result_value::<_, i32>());
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.name.value, "0");
    assert!(v.ty.is_none());
}

#[test]
fn test_ssa_value_numeric_name() {
    let result = test_parse!("%0", ssa_value::<_, i32>());
    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.name.value, "0");
    assert!(v.ty.is_none());
}

// === Block label edge cases ===

#[test]
fn test_block_label_long_name() {
    let result = test_parse!("^a_very_long_block_name_with_numbers_123", block_label());
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().name.value,
        "a_very_long_block_name_with_numbers_123"
    );
}

// === Identifier with numbers ===

#[test]
fn test_any_identifier_with_digits() {
    let result = test_parse!("abc123", any_identifier());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, "abc123");
}

// === typeof_ssa edge case ===

#[test]
fn test_typeof_ssa_empty() {
    let result: Result<crate::ast::TypeofSSAValue<i32>, _> =
        test_parse!("", typeof_ssa::<_, i32>());
    assert!(result.is_err(), "typeof_ssa should fail on empty input");
}

#[test]
fn test_typeof_ssa_non_type_token() {
    // A symbol is not a valid i32 type
    let result: Result<crate::ast::TypeofSSAValue<i32>, _> =
        test_parse!("@main", typeof_ssa::<_, i32>());
    assert!(result.is_err(), "typeof_ssa should fail on non-type token");
}

// === FunctionType equality ===

#[test]
fn test_function_type_ast_equality() {
    let ft1 = crate::ast::FunctionType {
        input_types: vec![crate::ast::Spanned::new(1i32, SimpleSpan::from(0..1))],
        output_types: vec![crate::ast::Spanned::new(2i32, SimpleSpan::from(5..6))],
    };
    let ft2 = crate::ast::FunctionType {
        input_types: vec![crate::ast::Spanned::new(1i32, SimpleSpan::from(99..100))],
        output_types: vec![crate::ast::Spanned::new(2i32, SimpleSpan::from(99..100))],
    };
    // FunctionType PartialEq delegates to Spanned PartialEq, which ignores span
    assert_eq!(ft1, ft2);
}

#[test]
fn test_function_type_ast_inequality() {
    let ft1 = crate::ast::FunctionType::<i32> {
        input_types: vec![crate::ast::Spanned::new(1, SimpleSpan::from(0..1))],
        output_types: vec![crate::ast::Spanned::new(2, SimpleSpan::from(5..6))],
    };
    let ft2 = crate::ast::FunctionType {
        input_types: vec![crate::ast::Spanned::new(1, SimpleSpan::from(0..1))],
        output_types: vec![crate::ast::Spanned::new(3, SimpleSpan::from(5..6))],
    };
    assert_ne!(ft1, ft2);
}

// === parse_ast with bool parser ===

#[test]
fn test_parse_ast_bool_true() {
    let result = crate::traits::parse_ast::<bool>("true");
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_parse_ast_bool_false() {
    let result = crate::traits::parse_ast::<bool>("false");
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[test]
fn test_parse_ast_bool_invalid() {
    let result = crate::traits::parse_ast::<bool>("maybe");
    assert!(result.is_err());
}

// === parse_ast with String parser ===

#[test]
fn test_parse_ast_string_identifier() {
    let result = crate::traits::parse_ast::<String>("hello");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "hello");
}

#[test]
fn test_parse_ast_string_quoted() {
    let result = crate::traits::parse_ast::<String>("\"world\"");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "world");
}

// === nameof_ssa empty input ===

#[test]
fn test_nameof_ssa_empty() {
    let result: Result<crate::ast::NameofSSAValue<'_>, _> = test_parse!("", nameof_ssa());
    assert!(result.is_err(), "nameof_ssa should fail on empty input");
}

// === block_argument_list edge cases ===

#[test]
fn test_block_argument_list_no_parens() {
    // Just an argument without parentheses should fail
    let result: Result<Vec<crate::ast::Spanned<crate::ast::BlockArgument<'_, i32>>>, _> =
        test_parse!("%a : 1", block_argument_list::<_, i32>());
    assert!(
        result.is_err(),
        "block argument list without parens should fail"
    );
}

#[test]
fn test_block_argument_list_missing_open_paren() {
    let result: Result<Vec<crate::ast::Spanned<crate::ast::BlockArgument<'_, i32>>>, _> =
        test_parse!("%a : 1 )", block_argument_list::<_, i32>());
    assert!(
        result.is_err(),
        "block argument list missing open paren should fail"
    );
}

// === FunctionParseError with_source chaining ===

#[test]
fn test_function_parse_error_chained_source() {
    use std::error::Error;

    let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let middle = crate::FunctionParseError::new(
        crate::FunctionParseErrorKind::BodyParseFailed,
        None,
        "body failed",
    )
    .with_source(inner);

    assert!(middle.source().is_some());
    let source = middle.source().unwrap();
    assert!(
        format!("{source}").contains("not found"),
        "source error message should propagate"
    );
}

// === ParseError clone ===

#[test]
fn test_parse_error_clone() {
    let err = crate::traits::ParseError {
        message: "test".to_string(),
        span: SimpleSpan::from(0..1),
    };
    let cloned = err.clone();
    assert_eq!(err.message, cloned.message);
    assert_eq!(err.span, cloned.span);
}

// === Multiple parse_ast error collection ===

#[test]
fn test_parse_ast_collects_errors_with_spans() {
    // Input that will generate errors with actual span info
    let result = crate::traits::parse_ast::<i32>("%not_an_int");
    assert!(result.is_err());
    let errors = result.unwrap_err();
    for e in &errors {
        // Spans should be valid (start <= end)
        assert!(e.span.start <= e.span.end);
    }
}
