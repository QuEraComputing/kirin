//! Integration tests for [`Quarantined`], the artifact a failed rewrite pass
//! leaves behind: its accessors, its diagnostic report, and its error impls.

mod common;

use common::{BuilderDialect, TestType, make_split, new_stage};
use kirin_ir::*;

struct Fixture {
    quarantine: Quarantined<StageInfo<BuilderDialect>>,
    add: Statement,
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
        quarantine: Quarantined::from_stage(stage, cause, events),
        add,
        nop,
    }
}

fn panicked() -> Quarantined<StageInfo<BuilderDialect>> {
    fixture(QuarantineCause::Panic("panic".to_string())).quarantine
}

/// The rows of one `report` section, excluding its title and column header.
fn section(report: &str, title: &str) -> Vec<String> {
    report
        .lines()
        .skip_while(|line| !line.starts_with(title))
        .skip(2)
        .take_while(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a `report` row carries is a tombstone. Checks the `del` column for "x"
fn is_deleted(row: &str) -> bool {
    row.split_whitespace().nth(1) == Some("x")
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
fn report_lists_every_id_including_tombstones() {
    let f = fixture(QuarantineCause::Panic("panic".to_string()));
    let report = f.quarantine.report();
    let rows = section(report, "== statements");

    // Four ids even though only three statements are live: erased entries are
    // listed, not skipped.
    assert!(report.contains("== statements (4 ids) =="), "{report}");
    assert_eq!(rows.len(), 4, "{rows:?}");

    // The erased statement keeps its own id and is flagged, and the statements
    // after it are *not* renumbered. This is what makes report ids line up
    // with the ids the event log names.
    assert!(rows[0].contains("Add("), "{rows:?}");
    assert!(is_deleted(&rows[1]) && rows[1].contains("Nop"), "{rows:?}");
    assert!(rows[2].contains("Split("), "{rows:?}");
    assert!(rows[3].contains("Return"), "{rows:?}");
    assert_eq!(rows.iter().filter(|row| is_deleted(row)).count(), 1);
    for (id, row) in rows.iter().enumerate() {
        assert_eq!(
            row.split_whitespace().next(),
            Some(id.to_string().as_str()),
            "{rows:?}"
        );
    }
}

#[test]
fn report_shows_both_mirrors_and_the_event_log() {
    let f = fixture(QuarantineCause::Panic("panic".to_string()));
    let report = f.quarantine.report();

    // Block section: the predecessor mirror plus the raw list summary,
    // reported without any link traversal.
    assert!(report.contains("== blocks (1 ids) =="), "{report}");
    assert!(report.contains("predecessors"), "{report}");

    // SSA section: the use mirror still points at the surviving `add`, and
    // both `SSAKind` shapes render without the `Id(..)` wrapper.
    let ssa_rows = section(report, "== ssa values");
    let add = f.add;
    assert!(
        ssa_rows
            .iter()
            .any(|row| row.contains(&format!("StatementOperand {{ stmt: {add:?}, index: 0 }}"))),
        "{ssa_rows:?}"
    );
    assert!(
        ssa_rows
            .iter()
            .any(|row| row.contains("BlockArgument(Block(0), 0)")),
        "{ssa_rows:?}"
    );
    assert!(
        ssa_rows
            .iter()
            .any(|row| row.contains("Result(Statement(2), 1)")),
        "{ssa_rows:?}"
    );
    // Builder placeholders discarded at finalize show as tombstoned rows.
    assert_eq!(ssa_rows.iter().filter(|row| is_deleted(row)).count(), 2);

    // The event log names the same id the statement table flagged.
    assert!(report.contains("== mutation events (1) =="), "{report}");
    let nop = f.nop;
    assert!(
        report.contains(&format!("ErasedStatement {{ stmt: {nop:?} }}")),
        "{report}"
    );
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
        Quarantined::from_stage(stage, QuarantineCause::Panic("desync".into()), vec![]);
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
    let nop = builder.statement().definition(BuilderDialect::Nop).new();
    let ret = builder.statement().definition(BuilderDialect::Return).new();
    let block = builder.block().stmt(nop).terminator(ret).new();
    let mut stage = builder.finalize().expect("fixture should finalize");

    let mut rewriter = Rewriter::new(&mut stage);
    rewriter.erase_statement(nop).expect("nop is erasable");
    drop(rewriter);
    block.get_info_mut(&mut stage).expect("live").terminator = Some(nop);

    let quarantine =
        Quarantined::from_stage(stage, QuarantineCause::Panic("dangling".into()), vec![]);
    let report = quarantine.report();
    let blocks = section(report, "== blocks");

    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert!(blocks[0].contains(&format!("{nop:?}")), "{blocks:?}");
    // The statement it names is listed, and marked deleted.
    let statements = section(report, "== statements");
    assert!(is_deleted(&statements[0]), "{statements:?}");
}

#[test]
fn debug_is_a_summary_not_a_dump() {
    // Cause and event count only — a derived impl would inline the payload's
    // arenas and the whole rendered report here. `..` points at both.
    assert_eq!(
        format!("{:?}", panicked()),
        "Quarantined { cause: Panic(\"panic\"), events: 1, .. }"
    );
}

#[test]
fn display_summarizes_the_failure() {
    assert_eq!(
        panicked().to_string(),
        "stage quarantined after 1 mutation event(s): the pass panicked: panic"
    );
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
        Quarantined::from_stage(stage, QuarantineCause::Panic("empty".to_string()), vec![]);
    let report = quarantine.report();

    for title in [
        "== statements (0 ids) ==",
        "== blocks (0 ids) ==",
        "== ssa values (0 ids) ==",
        "== mutation events (0) ==",
    ] {
        assert!(report.contains(title), "missing {title}:\n{report}");
    }
}
