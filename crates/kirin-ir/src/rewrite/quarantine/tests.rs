//! Rendering a stage whose derived metadata lies.
//!
//! In-crate because each test has to forge that state first, which no correct
//! [`Rewriter`](crate::Rewriter) can produce.
//!
//! What both assert: the report prints what is *installed*, never a fresh
//! derivation. A quarantine exists to show what the IR actually holds, so
//! recomputing would hide exactly the defect being diagnosed.

use crate::arena::GetInfo;
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

/// The rows of one report section, excluding its title and column header.
fn section(report: &str, title: &str) -> Vec<String> {
    report
        .lines()
        .skip_while(|line| !line.starts_with(title))
        .skip(2)
        .take_while(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a report row is a tombstone — the `del` column holds "x".
fn is_deleted(row: &str) -> bool {
    row.split_whitespace().nth(1) == Some("x")
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

    let real_arg: SSAValue = block.expect_info(&stage).arguments[0].into();
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
    let report = quarantine.report();

    assert!(
        report.contains(&format!("StatementOperand {{ stmt: {ret:?}, index: 9 }}")),
        "{report}"
    );
    // The block had no real predecessors, so the invented self-edge is the
    // only entry in the mirror.
    assert!(report.contains(&format!("[{block}]")), "{report}");
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
    let report = quarantine.report();
    let blocks = section(report, "== blocks");

    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert!(blocks[0].contains(&format!("{nop:?}")), "{blocks:?}");
    // The statement it names is listed, and marked deleted.
    let statements = section(report, "== statements");
    assert!(is_deleted(&statements[0]), "{statements:?}");
}
