//! Integration tests for [`Quarantined`], the artifact a failed rewrite pass
//! leaves behind: its accessors, its diagnostic report, and its error impls.

mod common;

use common::{BuilderDialect, TestType, make_split, new_stage};
use kirin_ir::*;

struct Fixture {
    quarantine: Quarantined<StageInfo<BuilderDialect>>,
    nop: Statement,
}

/// One block: two arguments, an `add` reading both, a `nop` that gets erased so
/// the arenas hold a tombstone, a `split` defining two results, and a `return`
/// terminator.
fn fixture(cause: QuarantineCause) -> Fixture {
    let mut builder = new_stage();
    let a = builder.block_argument().index(0);
    let b = builder.block_argument().index(1);
    let add = builder
        .statement()
        .definition(BuilderDialect::Add(a, b))
        .new();
    let nop = builder.statement().definition(BuilderDialect::Nop).new();
    let (split, _, _) = make_split(&mut builder);
    let ret = builder.statement().definition(BuilderDialect::Return).new();
    builder
        .block()
        .argument(TestType::I32)
        .argument(TestType::Any)
        .stmt(add)
        .stmt(nop)
        .stmt(split)
        .terminator(ret)
        .new();
    let mut stage = builder.finalize().expect("fixture should finalize");

    let mut rewriter = Rewriter::new(&mut stage);
    rewriter.erase_statement(nop).expect("nop is erasable");
    let events = rewriter.drain_events();

    Fixture {
        quarantine: Quarantined::from_stage_info(stage, cause, events),
        nop,
    }
}

fn panicked() -> Quarantined<StageInfo<BuilderDialect>> {
    fixture(QuarantineCause::Panic("panic".to_string())).quarantine
}

#[test]
fn cause_and_events_are_retained() {
    let f = fixture(QuarantineCause::Panic("panic".to_string()));

    assert!(matches!(
        f.quarantine.cause(),
        QuarantineCause::Panic(message) if message == "panic"
    ));
    // The edit made before the failure survives into the artifact.
    assert_eq!(f.quarantine.events().len(), 1);
    assert_eq!(
        f.quarantine.events()[0],
        MutationEvent::ErasedStatement { stmt: f.nop }
    );
}

#[test]
fn report_dumps_every_arena_and_the_event_log() {
    // What the snapshot pins down:
    //
    // - statements: four ids although only three are live. The erased `nop`
    //   keeps its id and is flagged `x`, and the statements after it are *not*
    //   renumbered, so report ids line up with the ids the event log names.
    // - blocks: the predecessor mirror plus the raw list summary, reported
    //   without any link traversal.
    // - ssa values: the use mirror still points at the surviving `add`, both
    //   `SSAKind` shapes render without the `Id(..)` wrapper, and the builder
    //   placeholders discarded at finalize show as tombstoned rows.
    // - mutation events: the erase names the same id the statement table
    //   flagged.
    insta::assert_snapshot!(panicked().report(), @r"
    == statements (4 ids) ==
       id  del  parent        definition
        0       Block(0)      Add(SSAValue(4), SSAValue(5))
        1   x   Block(0)      Nop
        2       Block(0)      Split(ResultValue(2), ResultValue(3))
        3       Block(0)      Return

    == blocks (1 ids) ==
       id  del  parent      predecessors      stmts h/t/len                             terminator
        0       -           []                Statement(0)/Statement(2)/2               Statement(3)

    == ssa values (6 ids) ==
       id  del  kind                            uses
        0   x   -                               -
        1   x   -                               -
        2       Result(Statement(2), 0)         []
        3       Result(Statement(2), 1)         []
        4       BlockArgument(Block(0), 0)      [StatementOperand { stmt: Statement(0), index: 0 }]
        5       BlockArgument(Block(0), 1)      [StatementOperand { stmt: Statement(0), index: 1 }]

    == mutation events (1) ==
        0  ErasedStatement { stmt: Statement(1) }
    ");
}

#[test]
fn report_renders_a_stage_whose_mirrors_are_desynced() {
    // The realistic `Verify(Mismatch)` shape: authoritative IR is intact but
    // the derived mirrors lie. `report` must print what is installed rather
    // than recomputing or trusting it.
    let mut builder = new_stage();
    let arg = builder.block_argument().index(0);
    let use_stmt = builder
        .statement()
        .definition(BuilderDialect::Use(arg))
        .new();
    let ret = builder.statement().definition(BuilderDialect::Return).new();
    let block = builder
        .block()
        .argument(TestType::I32)
        .stmt(use_stmt)
        .terminator(ret)
        .new();
    let mut stage = builder.finalize().expect("fixture should finalize");

    let real_arg: SSAValue = block.expect_info(&stage).arguments[0].into();
    // Invent a use that no operand slot backs.
    real_arg
        .get_info_mut(&mut stage)
        .expect("live")
        .uses_mut()
        .push(Use::StatementOperand {
            stmt: ret,
            index: 9,
        });
    // Invent a predecessor edge that no terminator backs.
    block
        .get_info_mut(&mut stage)
        .expect("live")
        .predecessors
        .push(block);

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
    let nop = builder.statement().definition(BuilderDialect::Nop).new();
    let ret = builder.statement().definition(BuilderDialect::Return).new();
    let block = builder.block().stmt(nop).terminator(ret).new();
    let mut stage = builder.finalize().expect("fixture should finalize");

    let mut rewriter = Rewriter::new(&mut stage);
    rewriter.erase_statement(nop).expect("nop is erasable");
    drop(rewriter);
    block.get_info_mut(&mut stage).expect("live").terminator = Some(nop);

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

#[test]
fn debug_is_a_summary_not_a_dump() {
    // Cause and event count only — a derived impl would inline the payload's
    // arenas and the whole rendered report here. `..` points at both.
    insta::assert_snapshot!(format!("{:?}", panicked()), @r#"Quarantined { cause: Panic("panic"), events: 1, .. }"#);
}

#[test]
fn display_summarizes_the_failure() {
    insta::assert_snapshot!(panicked(), @"stage quarantined after 1 mutation event(s): the pass panicked: panic");
}

#[test]
fn source_chains_to_a_pass_error_but_not_to_a_panic() {
    use std::error::Error as _;

    assert!(panicked().source().is_none(), "a panic has no error value");

    let f = fixture(QuarantineCause::Pass(Box::new(std::fmt::Error)));
    let source = f.quarantine.source().expect("pass errors have a source");
    assert_eq!(source.to_string(), std::fmt::Error.to_string());
    assert!(
        f.quarantine
            .to_string()
            .contains("the pass returned an error"),
        "{}",
        f.quarantine
    );
}

#[test]
fn empty_stage_reports_every_section() {
    let stage = new_stage().finalize().expect("an empty stage is valid");
    let quarantine =
        Quarantined::from_stage_info(stage, QuarantineCause::Panic("empty".to_string()), vec![]);

    insta::assert_snapshot!(quarantine.report(), @r"
    == statements (0 ids) ==
       id  del  parent        definition

    == blocks (0 ids) ==
       id  del  parent      predecessors      stmts h/t/len                             terminator

    == ssa values (0 ids) ==
       id  del  kind                            uses

    == mutation events (0) ==
    ");
}
