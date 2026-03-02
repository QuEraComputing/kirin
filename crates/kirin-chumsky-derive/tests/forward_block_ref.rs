//! Regression tests for forward block references in Region emit.
//!
//! Before the two-pass Region::emit fix, a `br ^bbN` that referenced a block
//! defined later in the same region would panic because the block had not yet
//! been registered in the EmitContext.

use kirin::ir::{Dialect, GetInfo, ResultValue, SSAValue, StageInfo, Successor};
use kirin_chumsky::{EmitContext, EmitIR, HasParser, PrettyPrint, parse_ast};
use kirin_test_languages::SimpleType;
use kirin_test_utils::new_test_ssa;

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum BranchLang {
    #[chumsky(format = "{res:name} = id {arg} -> {res:type}")]
    Id { res: ResultValue, arg: SSAValue },
    #[kirin(terminator)]
    #[chumsky(format = "br {target}")]
    Branch { target: Successor },
    #[chumsky(format = "{res} = scope {body}")]
    Scope {
        res: ResultValue,
        body: kirin::ir::Region,
    },
    #[kirin(terminator)]
    #[chumsky(format = "ret {0}")]
    Ret(SSAValue),
}

/// Forward reference: ^entry branches to ^exit which is defined after ^entry.
#[test]
fn test_region_forward_block_reference() {
    let input = r#"
        %r = scope {
            ^entry() {
                br ^exit;
            };
            ^exit() {
                ret %r;
            }
        }
    "#;

    let mut stage: StageInfo<BranchLang> = StageInfo::default();
    let ssa_r = new_test_ssa(&mut stage, "r", SimpleType::I32);

    let ast = parse_ast::<BranchLang>(input).expect("parse failed");
    let mut emit_ctx = EmitContext::new(&mut stage);
    emit_ctx.register_ssa("r".to_string(), ssa_r);

    // This should NOT panic — the two-pass emit registers ^exit before
    // emitting ^entry's body, so `br ^exit` resolves correctly.
    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&stage).expect("statement should exist");

    // Verify it's a Scope with a region containing two blocks
    match stmt_info.definition() {
        BranchLang::Scope { body, .. } => {
            let block_count = body.blocks(&stage).count();
            assert_eq!(block_count, 2, "region should contain 2 blocks");
        }
        _ => panic!("Expected Scope variant, got {:?}", stmt_info.definition()),
    }
}

/// Backward reference (^bb1 -> ^bb0) should still work with the two-pass approach.
#[test]
fn test_region_backward_block_reference() {
    let input = r#"
        %r = scope {
            ^bb0() {
                ret %r;
            };
            ^bb1() {
                br ^bb0;
            }
        }
    "#;

    let mut stage: StageInfo<BranchLang> = StageInfo::default();
    let ssa_r = new_test_ssa(&mut stage, "r", SimpleType::I32);

    let ast = parse_ast::<BranchLang>(input).expect("parse failed");
    let mut emit_ctx = EmitContext::new(&mut stage);
    emit_ctx.register_ssa("r".to_string(), ssa_r);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&stage).expect("statement should exist");

    match stmt_info.definition() {
        BranchLang::Scope { body, .. } => {
            let block_count = body.blocks(&stage).count();
            assert_eq!(block_count, 2, "region should contain 2 blocks");
        }
        _ => panic!("Expected Scope variant"),
    }
}

/// Three blocks with forward and backward references.
#[test]
fn test_region_mixed_references() {
    let input = r#"
        %r = scope {
            ^entry() {
                br ^middle;
            };
            ^middle() {
                br ^exit;
            };
            ^exit() {
                ret %r;
            }
        }
    "#;

    let mut stage: StageInfo<BranchLang> = StageInfo::default();
    let ssa_r = new_test_ssa(&mut stage, "r", SimpleType::I32);

    let ast = parse_ast::<BranchLang>(input).expect("parse failed");
    let mut emit_ctx = EmitContext::new(&mut stage);
    emit_ctx.register_ssa("r".to_string(), ssa_r);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&stage).expect("statement should exist");

    match stmt_info.definition() {
        BranchLang::Scope { body, .. } => {
            let block_count = body.blocks(&stage).count();
            assert_eq!(block_count, 3, "region should contain 3 blocks");
        }
        _ => panic!("Expected Scope variant"),
    }
}
