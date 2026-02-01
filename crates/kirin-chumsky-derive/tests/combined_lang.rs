//! Tests for the combined HasParser + PrettyPrint derive macros.

mod common;

use common::SimpleType;
use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::{parse_ast, HasParser, PrettyPrint};

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum CombinedLang {
    #[chumsky(format = "{res:name} = inc {arg} -> {res:type}")]
    Inc { res: ResultValue, arg: SSAValue },
    #[chumsky(format = "{res:name} = dec {arg} -> {res:type}")]
    Dec { res: ResultValue, arg: SSAValue },
}

#[test]
fn test_combined_derive_inc() {
    let ast = parse_ast::<CombinedLang>("%r = inc %x -> i32").expect("parse failed");
    match ast {
        CombinedLangAST::Inc { res, arg } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(arg.name.value, "x");
        }
        _ => panic!("Expected Inc variant"),
    }
}

#[test]
fn test_combined_derive_dec() {
    let ast = parse_ast::<CombinedLang>("%y = dec %z -> f64").expect("parse failed");
    match ast {
        CombinedLangAST::Dec { res, arg } => {
            assert_eq!(res.name.value, "y");
            assert_eq!(res.ty, Some(SimpleType::F64));
            assert_eq!(arg.name.value, "z");
        }
        _ => panic!("Expected Dec variant"),
    }
}

#[test]
fn test_multiple_variants_same_dialect() {
    let ast1 = parse_ast::<CombinedLang>("%r = inc %x -> i32").expect("parse failed");
    assert!(matches!(ast1, CombinedLangAST::Inc { .. }));

    let ast2 = parse_ast::<CombinedLang>("%y = dec %z -> f64").expect("parse failed");
    assert!(matches!(ast2, CombinedLangAST::Dec { .. }));
}
