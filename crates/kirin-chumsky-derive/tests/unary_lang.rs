//! Tests for ResultValue with :name only (no :type in format).

mod common;

use common::SimpleType;
use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::parse;
use kirin_chumsky_derive::{HasRecursiveParser, WithAbstractSyntaxTree};

#[derive(Debug, Clone, PartialEq, Dialect, HasRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum UnaryLang {
    #[chumsky(format = "{res:name} = neg {arg}")]
    Neg { res: ResultValue, arg: SSAValue },
    #[chumsky(format = "{res:name} = abs {arg} -> {res:type}")]
    Abs { res: ResultValue, arg: SSAValue },
}

#[test]
fn test_result_name_only() {
    let ast = parse::<UnaryLang>("%x = neg %y").expect("parse failed");
    match ast {
        UnaryLangAST::Neg { res, arg } => {
            assert_eq!(res.name.value, "x");
            assert!(res.ty.is_none(), "Expected ty to be None for :name only");
            assert_eq!(arg.name.value, "y");
        }
        _ => panic!("Expected Neg variant"),
    }
}

#[test]
fn test_result_name_and_type() {
    let ast = parse::<UnaryLang>("%x = abs %y -> i32").expect("parse failed");
    match ast {
        UnaryLangAST::Abs { res, arg } => {
            assert_eq!(res.name.value, "x");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(arg.name.value, "y");
        }
        _ => panic!("Expected Abs variant"),
    }
}
