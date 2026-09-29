//! Rendering a stage whose derived metadata lies.
//!
//! In-crate because each test has to forge that state first, which no correct
//! [`Rewriter`](crate::Rewriter) can produce.
//!
//! What both assert: the report prints what is *installed*, never a fresh
//! derivation. A quarantine exists to show what the IR actually holds, so
//! recomputing would hide exactly the defect being diagnosed.

use crate::arena::{GetInfo, GetInfoMut};
use crate::testing::dialect::{TestLang, TestType, new_stage};
use crate::{Block, QuarantineCause, Quarantined, Rewriter, SSAValue, StageInfo, Statement, Use};

/// Add a use that no operand slot backs.
///
/// None of the three below can be produced by a `Rewriter` edit; they are
/// forged so the report has a desynced mirror to render.
fn push_use(stage: &mut StageInfo<TestLang>, value: SSAValue, site: Use) {
    value
        .get_info_mut(stage)
        .expect("live SSA value")
        .uses
        .push(site);
}

/// Add a predecessor edge that no terminator backs.
fn push_predecessor(stage: &mut StageInfo<TestLang>, block: Block, predecessor: Block) {
    block
        .get_info_mut(stage)
        .expect("live block")
        .predecessors
        .push(predecessor);
}

/// Point a block's cached terminator somewhere the block body does not.
fn set_terminator(stage: &mut StageInfo<TestLang>, block: Block, terminator: Option<Statement>) {
    block.get_info_mut(stage).expect("live block").terminator = terminator;
}

#[test]
fn report_renders_a_stage_whose_mirrors_are_desynced() {
    // The realistic `Verify(Mismatch)` shape: authoritative IR is intact but
    // the derived mirrors lie.
    let mut builder = new_stage();
    let arg = builder.block_argument().index(0);
    let use_stmt = builder.statement().definition(TestLang::Use(arg)).new();
    let ret = builder.statement().definition(TestLang::Return).new();
    let block = builder
        .block()
        .argument(TestType::I32)
        .stmt(use_stmt)
        .terminator(ret)
        .new();
    let mut stage = builder.finalize().expect("fixture should finalize");

    let real_arg: SSAValue = block.expect_info(&stage).arguments()[0].into();
    // Invent a use that no operand slot backs.
    push_use(
        &mut stage,
        real_arg,
        Use::StatementOperand {
            stmt: ret,
            index: 9,
        },
    );
    // Invent a predecessor edge that no terminator backs.
    push_predecessor(&mut stage, block, block);

    let quarantine =
        Quarantined::from_stage_info(stage, QuarantineCause::Panic("desync".into()), vec![]);

    // The invented operand use (index 9 on the `return`) and the invented
    // self-edge both appear; the block had no real predecessors, so the
    // self-edge is the only entry in the mirror.
    insta::assert_snapshot!(quarantine.report(), @r"
    == statements (2 ids) ==
       id  del  parent        definition
        0       Block(0)      Use(SSAValue(1))
        1       Block(0)      Return

    == blocks (1 ids) ==
       id  del  parent      predecessors      stmts h/t/len                             terminator
        0       -           [^0]              Statement(0)/Statement(0)/1               Statement(1)

    == ssa values (2 ids) ==
       id  del  kind                            uses
        0   x   -                               -
        1       BlockArgument(Block(0), 0)      [StatementOperand { stmt: Statement(0), index: 0 }, StatementOperand { stmt: Statement(1), index: 9 }]

    == mutation events (0) ==
    ");
}

#[test]
fn report_tolerates_a_terminator_pointing_at_a_tombstone() {
    // Erasing through the `Rewriter` cannot produce this, but a bypassing
    // mutation can; the report must still render the block row.
    let mut builder = new_stage();
    let nop = builder.statement().definition(TestLang::Nop).new();
    let ret = builder.statement().definition(TestLang::Return).new();
    let block = builder.block().stmt(nop).terminator(ret).new();
    let mut stage = builder.finalize().expect("fixture should finalize");

    let mut rewriter = Rewriter::new(&mut stage);
    rewriter.erase_statement(nop).expect("nop is erasable");
    drop(rewriter);
    set_terminator(&mut stage, block, Some(nop));

    let quarantine =
        Quarantined::from_stage_info(stage, QuarantineCause::Panic("dangling".into()), vec![]);

    // The block's terminator names the erased statement, which is listed and
    // marked deleted.
    insta::assert_snapshot!(quarantine.report(), @r"
    == statements (2 ids) ==
       id  del  parent        definition
        0   x   Block(0)      Nop
        1       Block(0)      Return

    == blocks (1 ids) ==
       id  del  parent      predecessors      stmts h/t/len                             terminator
        0       -           []                -/-/0                                     Statement(0)

    == ssa values (0 ids) ==
       id  del  kind                            uses

    == mutation events (0) ==
    ");
}
