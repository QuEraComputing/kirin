//! Integration tests for PrettyPrint derive macro.
//!
//! These tests verify that the PrettyPrint derive macro generates correct
//! implementations that can be used with the pretty printing infrastructure.

use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::{HasParser, PrettyPrint};
use kirin_test_languages::SimpleType;

// A simple dialect for testing pretty print derive
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum TestLang {
    #[chumsky(format = "{res:name} = add {lhs}, {rhs} -> {res:type}")]
    Add {
        res: ResultValue,
        lhs: SSAValue,
        rhs: SSAValue,
    },
    #[chumsky(format = "return {0}")]
    Return(SSAValue),
}

/// Test that the PrettyPrint derive generates a valid implementation.
/// This is a compile-time test - if it compiles, the derive worked.
#[test]
fn test_pretty_print_derive_compiles() {
    // Verify that TestLang implements PrettyPrint
    fn assert_pretty_print<T: PrettyPrint>() {}
    assert_pretty_print::<TestLang>();
}

/// Test that the generated AST type exists (compile-time test).
#[test]
fn test_ast_type_exists() {
    // The HasParser derive generates TestLangAST
    // We just verify the type exists - actual parsing is tested elsewhere
    fn _verify_type_exists() -> Option<
        TestLangAST<'static, 'static, SimpleType, TestLangASTSelf<'static, 'static, SimpleType>>,
    > {
        None
    }
}
