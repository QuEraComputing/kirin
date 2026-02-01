//! Tests for Block and Region field parsing.

mod common;

use common::SimpleType;
use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::parse;
use kirin_chumsky_derive::{HasRecursiveParser, WithAbstractSyntaxTree};

#[derive(Debug, Clone, PartialEq, Dialect, HasRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum BlockRegionLang {
    #[chumsky(format = "{res:name} = id {arg} -> {res:type}")]
    Id { res: ResultValue, arg: SSAValue },
    #[chumsky(format = "{res} = loop {body}")]
    Loop {
        res: ResultValue,
        body: kirin::ir::Block,
    },
    #[chumsky(format = "{res} = scope {body}")]
    Scope {
        res: ResultValue,
        body: kirin::ir::Region,
    },
    #[chumsky(format = "ret {0}")]
    Ret(SSAValue),
}

// === Block Tests ===

#[test]
fn test_block_empty_body() {
    let ast = parse::<BlockRegionLang>("%out = loop ^entry() { }").expect("parse failed");
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, None);
            assert_eq!(body.value.header.value.label.name.value, "entry");
            assert!(body.value.header.value.arguments.is_empty());
            assert!(body.value.statements.is_empty());
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_block_with_arguments() {
    let ast = parse::<BlockRegionLang>("%res: bool = loop ^bb0(%x: i32, %y: f64) { }").expect("parse failed");
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "res");
            assert_eq!(res.ty, Some(SimpleType::Bool));
            assert_eq!(body.value.header.value.label.name.value, "bb0");
            let args = &body.value.header.value.arguments;
            assert_eq!(args.len(), 2);
            assert_eq!(args[0].value.name.value, "x");
            assert_eq!(args[0].value.ty.value, SimpleType::I32);
            assert_eq!(args[1].value.name.value, "y");
            assert_eq!(args[1].value.ty.value, SimpleType::F64);
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_block_with_statements() {
    let ast =
        parse::<BlockRegionLang>("%res: i64 = loop ^body(%n: i32) { %r = id %n -> i32; }").expect("parse failed");
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "res");
            assert_eq!(res.ty, Some(SimpleType::I64));
            assert_eq!(body.value.statements.len(), 1);
            match &body.value.statements[0].value {
                BlockRegionLangAST::Id { res, arg } => {
                    assert_eq!(res.name.value, "r");
                    assert_eq!(res.ty, Some(SimpleType::I32));
                    assert_eq!(arg.name.value, "n");
                }
                _ => panic!("Expected Id statement"),
            }
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_block_with_multiple_statements() {
    let ast =
        parse::<BlockRegionLang>("%res: unit = loop ^main(%a: i32) { %b = id %a -> i32; %c = id %b -> f32; }")
            .expect("parse failed");
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "res");
            assert_eq!(res.ty, Some(SimpleType::Unit));
            assert_eq!(body.value.statements.len(), 2);
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_block_argument_with_all_types() {
    let ast = parse::<BlockRegionLang>(
        "%out: i32 = loop ^bb0(%a: i32, %b: i64, %c: f32, %d: f64, %e: bool, %f: unit) { }",
    )
    .expect("parse failed");
    match ast {
        BlockRegionLangAST::Loop { body, .. } => {
            let args = &body.value.header.value.arguments;
            assert_eq!(args.len(), 6);
            assert_eq!(args[0].value.ty.value, SimpleType::I32);
            assert_eq!(args[1].value.ty.value, SimpleType::I64);
            assert_eq!(args[2].value.ty.value, SimpleType::F32);
            assert_eq!(args[3].value.ty.value, SimpleType::F64);
            assert_eq!(args[4].value.ty.value, SimpleType::Bool);
            assert_eq!(args[5].value.ty.value, SimpleType::Unit);
        }
        _ => panic!("Expected Loop variant"),
    }
}

// === Region Tests ===

#[test]
fn test_region_empty() {
    let ast = parse::<BlockRegionLang>("%out: i32 = scope { }").expect("parse failed");
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert!(body.blocks.is_empty());
        }
        _ => panic!("Expected Scope variant"),
    }
}

#[test]
fn test_region_single_block() {
    let ast = parse::<BlockRegionLang>("%out: f32 = scope { ^entry() { }; }").expect("parse failed");
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::F32));
            assert_eq!(body.blocks.len(), 1);
            assert_eq!(body.blocks[0].value.header.value.label.name.value, "entry");
        }
        _ => panic!("Expected Scope variant"),
    }
}

#[test]
fn test_region_multiple_blocks() {
    let ast = parse::<BlockRegionLang>("%out: bool = scope { ^bb0(%x: i32) { }; ^bb1() { }; ^exit() { }; }")
        .expect("parse failed");
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::Bool));
            assert_eq!(body.blocks.len(), 3);
            assert_eq!(body.blocks[0].value.header.value.label.name.value, "bb0");
            assert_eq!(body.blocks[1].value.header.value.label.name.value, "bb1");
            assert_eq!(body.blocks[2].value.header.value.label.name.value, "exit");
        }
        _ => panic!("Expected Scope variant"),
    }
}

#[test]
fn test_region_with_statements_in_blocks() {
    let ast = parse::<BlockRegionLang>(
        "%out: unit = scope { ^bb0(%a: i32) { %b = id %a -> i64; }; ^bb1() { %c = id %b -> f32; }; }",
    )
    .expect("parse failed");
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(body.blocks.len(), 2);
            assert_eq!(body.blocks[0].value.statements.len(), 1);
            assert_eq!(body.blocks[1].value.statements.len(), 1);
        }
        _ => panic!("Expected Scope variant"),
    }
}

#[test]
fn test_region_without_trailing_semicolon() {
    let ast = parse::<BlockRegionLang>("%out: i32 = scope { ^only() { } }").expect("parse failed");
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(body.blocks.len(), 1);
        }
        _ => panic!("Expected Scope variant"),
    }
}

// === Deep Nesting Tests ===

#[test]
fn test_nested_loop_in_scope() {
    let ast = parse::<BlockRegionLang>("%out: unit = scope { ^bb0() { %inner_res: i32 = loop ^inner() { }; }; }")
        .expect("parse failed");
    match ast {
        BlockRegionLangAST::Scope { body, .. } => {
            match &body.blocks[0].value.statements[0].value {
                BlockRegionLangAST::Loop { res, body } => {
                    assert_eq!(res.name.value, "inner_res");
                    assert_eq!(body.value.header.value.label.name.value, "inner");
                }
                _ => panic!("Expected Loop statement"),
            }
        }
        _ => panic!("Expected Scope variant"),
    }
}

#[test]
fn test_nested_scope_in_loop() {
    let ast = parse::<BlockRegionLang>("%out: unit = loop ^outer() { %inner_res: i32 = scope { ^inner() { } }; }")
        .expect("parse failed");
    match ast {
        BlockRegionLangAST::Loop { body, .. } => {
            match &body.value.statements[0].value {
                BlockRegionLangAST::Scope { res, body } => {
                    assert_eq!(res.name.value, "inner_res");
                    assert_eq!(body.blocks[0].value.header.value.label.name.value, "inner");
                }
                _ => panic!("Expected Scope statement"),
            }
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_deeply_nested_structure() {
    let ast = parse::<BlockRegionLang>(
        "%out: unit = scope { ^bb0() { %loop_res: i64 = loop ^loop0() { %scope_res: bool = scope { ^bb1() { } }; }; }; }",
    )
    .expect("parse failed");
    match ast {
        BlockRegionLangAST::Scope { body, .. } => {
            let bb0 = &body.blocks[0].value;
            match &bb0.statements[0].value {
                BlockRegionLangAST::Loop { res, body } => {
                    assert_eq!(res.name.value, "loop_res");
                    match &body.value.statements[0].value {
                        BlockRegionLangAST::Scope { res, body } => {
                            assert_eq!(res.name.value, "scope_res");
                            assert_eq!(body.blocks[0].value.header.value.label.name.value, "bb1");
                        }
                        _ => panic!("Expected nested Scope"),
                    }
                }
                _ => panic!("Expected Loop"),
            }
        }
        _ => panic!("Expected Scope variant"),
    }
}

// === Error Cases ===

#[test]
fn test_block_missing_label() {
    assert!(parse::<BlockRegionLang>("%out = loop () { }").is_err());
}

#[test]
fn test_block_missing_braces() {
    assert!(parse::<BlockRegionLang>("%out = loop ^bb0()").is_err());
}
