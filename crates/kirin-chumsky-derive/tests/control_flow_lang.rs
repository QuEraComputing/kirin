//! Tests for Successor field parsing.

use kirin::ir::{Dialect, ResultValue, SSAValue, Successor};
use kirin_chumsky::{HasParser, PrettyPrint, parse_ast};
use kirin_test_languages::SimpleType;

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum ControlFlowLang {
    #[chumsky(format = "{res:name} = id {arg} -> {res:type}")]
    Id { res: ResultValue, arg: SSAValue },
    #[chumsky(format = "br {target}")]
    Branch { target: Successor },
    #[chumsky(format = "cond_br {cond} then = {true_target} else = {false_target}")]
    CondBranch {
        cond: SSAValue,
        true_target: Successor,
        false_target: Successor,
    },
}

#[test]
fn test_successor_branch() {
    let ast = parse_ast::<ControlFlowLang>("br ^exit").expect("parse failed");
    match ast.0 {
        ControlFlowLangAST::Branch { target, .. } => {
            assert_eq!(target.name.value, "exit");
        }
        _ => panic!("Expected Branch variant"),
    }
}

#[test]
fn test_successor_cond_branch() {
    let ast = parse_ast::<ControlFlowLang>("cond_br %flag then = ^bb1 else = ^bb2")
        .expect("parse failed");
    match ast.0 {
        ControlFlowLangAST::CondBranch {
            cond,
            true_target,
            false_target,
            ..
        } => {
            assert_eq!(cond.name.value, "flag");
            assert_eq!(true_target.name.value, "bb1");
            assert_eq!(false_target.name.value, "bb2");
        }
        _ => panic!("Expected CondBranch variant"),
    }
}

#[test]
fn test_successor_numeric_label() {
    let ast = parse_ast::<ControlFlowLang>("br ^0").expect("parse failed");
    match ast.0 {
        ControlFlowLangAST::Branch { target, .. } => {
            assert_eq!(target.name.value, "0");
        }
        _ => panic!("Expected Branch variant"),
    }
}

#[test]
fn test_successor_missing_caret() {
    assert!(parse_ast::<ControlFlowLang>("br exit").is_err());
}
