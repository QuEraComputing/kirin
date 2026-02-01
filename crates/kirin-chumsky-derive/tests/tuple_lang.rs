//! Tests for tuple variants with positional fields.

mod common;

use common::SimpleType;
use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::parse;
use kirin_chumsky_derive::{HasRecursiveParser, WithAbstractSyntaxTree};

#[derive(Debug, Clone, PartialEq, Dialect, HasRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum TupleLang {
    #[chumsky(format = "swap {0} {1}")]
    Swap(SSAValue, SSAValue),
    #[chumsky(format = "{res:name} = sel {cond} {left} {right} -> {res:type}")]
    Select {
        res: ResultValue,
        cond: SSAValue,
        left: SSAValue,
        right: SSAValue,
    },
}

#[test]
fn test_tuple_two_positional() {
    let ast = parse::<TupleLang>("swap %a %b").expect("parse failed");
    match ast {
        TupleLangAST::Swap(first, second) => {
            assert_eq!(first.name.value, "a");
            assert_eq!(second.name.value, "b");
        }
        _ => panic!("Expected Swap variant"),
    }
}

#[test]
fn test_named_fields_four_fields() {
    let ast = parse::<TupleLang>("%out = sel %cond %left %right -> i32").expect("parse failed");
    match ast {
        TupleLangAST::Select {
            res,
            cond,
            left,
            right,
        } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(cond.name.value, "cond");
            assert_eq!(left.name.value, "left");
            assert_eq!(right.name.value, "right");
        }
        _ => panic!("Expected Select variant"),
    }
}
