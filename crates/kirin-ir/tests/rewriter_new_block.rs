//! `Rewriter::new_block` makes a block owned by nothing, to become a
//! statement's body; one still unowned when the `Rewriter` drops is
//! tombstoned.

mod common;

use common::{BuilderDialect, TestType, new_stage};
use kirin_ir::*;

/// `owner = own_blocks { } { }` in `outer`, whose argument `x` the new blocks
/// read.
struct Fixture {
    stage: StageInfo<BuilderDialect>,
    owner: Statement,
    old_then: Block,
    old_else: Block,
    x: SSAValue,
}

fn fixture() -> Fixture {
    let mut stage = new_stage();
    let old_then = stage.block().new();
    let old_else = stage.block().new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(old_then, old_else))
        .new();
    let outer = stage.block().argument(TestType::I32).stmt(owner).new();
    let stage = stage.finalize().unwrap();
    let x = outer.expect_info(&stage).arguments()[0].into();
    Fixture {
        stage,
        owner,
        old_then,
        old_else,
        x,
    }
}

/// Fill `block` with `use(x)` then `return`.
fn fill(rewriter: &mut Rewriter<'_, BuilderDialect>, block: Block, x: SSAValue) -> Statement {
    let ret = rewriter
        .insert_terminator(block, BuilderDialect::Return)
        .unwrap();
    rewriter.insert_before(ret, BuilderDialect::Use(x)).unwrap()
}

#[test]
fn new_block_has_its_arguments_and_no_parent() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let block = rewriter.new_block([TestType::I32, TestType::I64]);

    let info = block.expect_info(rewriter.stage());
    assert_eq!(info.parent(), None);
    let types: Vec<TestType> = info
        .arguments()
        .iter()
        .map(|&arg| {
            SSAValue::from(arg)
                .expect_info(rewriter.stage())
                .ty()
                .clone()
        })
        .collect();
    assert_eq!(types, [TestType::I32, TestType::I64]);
    assert_eq!(
        rewriter.events(),
        [],
        "a block owned by nothing is not part of the IR yet"
    );
}

#[test]
fn adopted_new_blocks_become_the_owners_bodies() {
    let mut f = fixture();
    let (then_block, then_use, else_block, else_use) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let then_block = rewriter.new_block([]);
        let then_use = fill(&mut rewriter, then_block, f.x);
        let else_block = rewriter.new_block([]);
        let else_use = fill(&mut rewriter, else_block, f.x);
        rewriter
            .replace_statement(f.owner, BuilderDialect::OwnBlocks(then_block, else_block))
            .unwrap();
        (then_block, then_use, else_block, else_use)
    };

    for (block, stmt) in [(then_block, then_use), (else_block, else_use)] {
        assert_eq!(
            block.expect_info(&f.stage).parent(),
            Some(BlockParent::Statement(f.owner))
        );
        assert!(
            stmt.get_info(&f.stage).is_some(),
            "the drop keeps owned blocks"
        );
    }
    for block in [f.old_then, f.old_else] {
        assert!(
            block.get_info(&f.stage).is_none(),
            "the replaced bodies are erased"
        );
    }
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn unowned_new_block_is_tombstoned_on_drop() {
    let mut f = fixture();
    let (block, arg, used, events) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let block = rewriter.new_block([TestType::I32]);
        let arg: SSAValue = block.expect_info(rewriter.stage()).arguments()[0].into();
        let used = fill(&mut rewriter, block, f.x);
        (block, arg, used, rewriter.drain_events())
    };

    assert!(block.get_info(&f.stage).is_none());
    assert!(arg.get_info(&f.stage).is_none());
    assert!(used.get_info(&f.stage).is_none());
    assert!(
        f.x.expect_info(&f.stage).uses().is_empty(),
        "the tombstoned statement's use of `x` is dropped"
    );
    // Statements were inserted into it, so events were recorded; nothing
    // records the block's removal, since it never joined the IR.
    assert!(events.iter().all(|e| !matches!(
        e,
        MutationEvent::ChangedBlockList { .. } | MutationEvent::ChangedBodyNesting { .. }
    )));
    assert_eq!(verify_derived(&f.stage), Ok(()));
}
