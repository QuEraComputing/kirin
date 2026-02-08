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
