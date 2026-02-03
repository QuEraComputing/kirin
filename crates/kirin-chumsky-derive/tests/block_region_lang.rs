//! Tests for Block and Region field parsing.

use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::{HasParser, PrettyPrint, parse_ast};
use kirin_test_utils::SimpleType;

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
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

#[test]
fn test_parse_loop_with_block() {
    let input = r#"
        %r = loop ^entry(%x: i32) {
            ret %x;
        }
    "#;

    let result = parse_ast::<BlockRegionLang>(input);
    assert!(
        result.is_ok(),
        "Failed to parse loop with block: {:?}",
        result
    );
}

#[test]
fn test_parse_scope_with_region() {
    let input = r#"
        %r = scope {
            ^entry(%x: i32) {
                ret %x;
            }
        }
    "#;

    let result = parse_ast::<BlockRegionLang>(input);
    assert!(
        result.is_ok(),
        "Failed to parse scope with region: {:?}",
        result
    );
}
