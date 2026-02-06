//! Tests for basic dialect parsing with SSAValue and ResultValue fields.

use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::{HasParser, PrettyPrint, parse_ast};
use kirin_test_utils::SimpleType;

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum TestLang {
    #[chumsky(format = "{res:name} = add {lhs} {rhs} -> {res:type}")]
    Add {
        res: ResultValue,
        lhs: SSAValue,
        rhs: SSAValue,
    },
    #[chumsky(format = "{res:name} = mul {lhs:name}: {lhs:type}, {rhs} -> {res:type}")]
    Mul {
        res: ResultValue,
        lhs: SSAValue,
        rhs: SSAValue,
    },
    #[chumsky(format = "return {0}")]
    Return(SSAValue),
}

#[test]
fn test_parse_add() {
    let ast = parse_ast::<TestLang>("%result = add %a %b -> i32").expect("parse failed");
    match ast.0 {
        TestLangAST::Add { res, lhs, rhs, .. } => {
            assert_eq!(res.name.value, "result");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(lhs.name.value, "a");
            assert_eq!(rhs.name.value, "b");
        }
        _ => panic!("Expected Add variant"),
    }
}

#[test]
fn test_parse_mul() {
    let ast = parse_ast::<TestLang>("%x = mul %y: i32, %z -> i64").expect("parse failed");
    match ast.0 {
        TestLangAST::Mul { res, lhs, rhs, .. } => {
            assert_eq!(res.name.value, "x");
            assert_eq!(res.ty, Some(SimpleType::I64));
            assert_eq!(lhs.name.value, "y");
            assert_eq!(lhs.ty, Some(SimpleType::I32));
            assert_eq!(rhs.name.value, "z");
        }
        _ => panic!("Expected Mul variant"),
    }
}

#[test]
fn test_parse_return() {
    let ast = parse_ast::<TestLang>("return %value").expect("parse failed");
    match ast.0 {
        TestLangAST::Return(ssa, ..) => {
            assert_eq!(ssa.name.value, "value");
        }
        _ => panic!("Expected Return variant"),
    }
}

#[test]
fn test_parse_fails_on_invalid_input() {
    assert!(parse_ast::<TestLang>("invalid syntax here").is_err());
}

#[test]
fn test_parse_ssa_default_with_type() {
    let ast = parse_ast::<TestLang>("return %x: i32").expect("parse failed");
    match ast.0 {
        TestLangAST::Return(ssa, ..) => {
            assert_eq!(ssa.name.value, "x");
            assert_eq!(ssa.ty, Some(SimpleType::I32));
        }
        _ => panic!("Expected Return variant"),
    }
}

#[test]
fn test_parse_ssa_default_without_type() {
    let ast = parse_ast::<TestLang>("return %x").expect("parse failed");
    match ast.0 {
        TestLangAST::Return(ssa, ..) => {
            assert_eq!(ssa.name.value, "x");
            assert!(ssa.ty.is_none());
        }
        _ => panic!("Expected Return variant"),
    }
}
