//! Integration tests for the Rewriter mutation layer: operand rewriting and
//! mutation-event recording.

mod common;

use std::collections::HashSet;

use common::{BuilderDialect, TestType, make_wire, new_stage};
use kirin_ir::*;

/// The def-use set of `value` (order-agnostic; `SSAInfo::uses` is a bag).
fn uses_set(stage: &StageInfo<BuilderDialect>, value: impl Into<SSAValue>) -> HashSet<Use> {
    let value: SSAValue = value.into();
    value
        .get_info(stage)
        .map(|info| info.uses().iter().copied().collect())
        .unwrap_or_default()
}

fn so(stmt: Statement, index: usize) -> Use {
    Use::StatementOperand { stmt, index }
}

fn dgy(graph: DiGraph, index: usize) -> Use {
    Use::DiGraphYield { graph, index }
}

#[test]
fn replace_all_uses_rewrites_operands_and_records_events() {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let y = stage.block_argument().index(1);
    let add = stage
        .statement()
        .definition(BuilderDialect::Add(x, x))
        .new();
    let use_stmt = stage.statement().definition(BuilderDialect::Use(x)).new();
    let _keep_y = stage.statement().definition(BuilderDialect::Use(y)).new();

    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(add)
        .stmt(use_stmt)
        .stmt(_keep_y)
        .new();

    let mut stage = stage.finalize().unwrap();
    let (real_x, real_y) = {
        let info = block.expect_info(&stage);
        (
            SSAValue::from(info.arguments[0]),
            SSAValue::from(info.arguments[1]),
        )
    };

    let mut rewriter = Rewriter::new(&mut stage);
    let replaced = rewriter.replace_all_uses(real_x, real_y).unwrap();
    assert_eq!(replaced, 3, "Add uses x twice, Use once");
    assert_eq!(
        rewriter.drain_events(),
        vec![
            MutationEvent::ChangedOperands { stmt: add },
            MutationEvent::ChangedOperands { stmt: use_stmt },
            MutationEvent::ReplacedUses {
                old: real_x,
                new: real_y,
            },
        ],
        "one ChangedOperands per affected statement, then one ReplacedUses"
    );

    // A second replacement finds nothing left to rewrite and records nothing.
    assert_eq!(rewriter.replace_all_uses(real_x, real_y).unwrap(), 0);
    assert!(rewriter.events().is_empty());

    match add.definition(&stage) {
        BuilderDialect::Add(a, b) => {
            assert_eq!(*a, real_y);
            assert_eq!(*b, real_y);
        }
        other => panic!("expected Add, got {other:?}"),
    }
    match use_stmt.definition(&stage) {
        BuilderDialect::Use(a) => assert_eq!(*a, real_y),
        other => panic!("expected Use, got {other:?}"),
    }
}

#[test]
fn set_operand_updates_single_slot_and_rejects_illegal_edits() {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let add = stage
        .statement()
        .definition(BuilderDialect::Add(x, x))
        .new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I64)
        .stmt(add)
        .new();

    let mut stage = stage.finalize().unwrap();
    let (real_x, real_y) = {
        let info = block.expect_info(&stage);
        (
            SSAValue::from(info.arguments[0]),
            SSAValue::from(info.arguments[1]),
        )
    };

    let mut rewriter = Rewriter::new(&mut stage);

    // Rewrite only the second operand; the first must stay untouched.
    let old = rewriter.set_operand(add, 1, real_y).unwrap();
    assert_eq!(old, real_x);

    // Out-of-range operand index is rejected.
    assert_eq!(
        rewriter.set_operand(add, 2, real_y),
        Err(RewriteError::OperandIndexOutOfRange {
            stmt: add,
            index: 2
        })
    );

    // A value id that does not resolve to a live SSA value is rejected.
    let bogus = SSAValue::from(TestSSAValue(9999));
    assert_eq!(
        rewriter.set_operand(add, 0, bogus),
        Err(RewriteError::UnknownValue(bogus))
    );

    // Writing the value already in the slot is a no-op and records no event.
    assert_eq!(rewriter.set_operand(add, 1, real_y).unwrap(), real_y);

    assert_eq!(
        rewriter.drain_events(),
        vec![MutationEvent::ChangedOperands { stmt: add }],
        "only the one real edit is recorded"
    );

    match add.definition(&stage) {
        BuilderDialect::Add(a, b) => {
            assert_eq!(*a, real_x, "operand 0 untouched");
            assert_eq!(*b, real_y, "operand 1 rewritten");
        }
        other => panic!("expected Add, got {other:?}"),
    }
}

#[test]
fn set_operand_maintains_def_use_index() {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let add = stage
        .statement()
        .definition(BuilderDialect::Add(x, x))
        .new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(add)
        .new();

    let mut stage = stage.finalize().unwrap();
    let (real_x, real_y) = {
        let info = block.expect_info(&stage);
        (
            SSAValue::from(info.arguments[0]),
            SSAValue::from(info.arguments[1]),
        )
    };

    // Before: x is read at both operand slots of `add`; y is unread.
    assert_eq!(
        uses_set(&stage, real_x),
        HashSet::from([so(add, 0), so(add, 1)])
    );
    assert!(uses_set(&stage, real_y).is_empty());
    // use-def (each value's defining kind) — must be unchanged by the edit.
    let x_kind = *real_x.get_info(&stage).unwrap().kind();
    let y_kind = *real_y.get_info(&stage).unwrap().kind();

    {
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter.set_operand(add, 1, real_y).unwrap();
    }

    // After: the {add, 1} use moved from x to y; {add, 0} still reads x.
    assert_eq!(uses_set(&stage, real_x), HashSet::from([so(add, 0)]));
    assert_eq!(uses_set(&stage, real_y), HashSet::from([so(add, 1)]));
    // use-def is untouched — set_operand changes who reads a value, not its def.
    assert_eq!(*real_x.get_info(&stage).unwrap().kind(), x_kind);
    assert_eq!(*real_y.get_info(&stage).unwrap().kind(), y_kind);

    // The incrementally maintained index equals a from-scratch rebuild.
    let (mx, my) = (uses_set(&stage, real_x), uses_set(&stage, real_y));
    stage.rebuild_use_index();
    assert_eq!(uses_set(&stage, real_x), mx);
    assert_eq!(uses_set(&stage, real_y), my);
}

#[test]
fn replace_all_uses_maintains_def_use_index_including_yields() {
    let mut stage = new_stage();

    // `%a` is produced by s_src, read by `consumer`'s operand, and yielded by
    // the digraph — an operand use and a boundary-yield use of the same value.
    let s_src = stage.statement().definition(BuilderDialect::Nop).new();
    let a = stage
        .ssa()
        .ty(TestType::I32)
        .kind(BuilderSSAKind::Result(s_src, 0))
        .new();
    let consumer = stage.statement().definition(BuilderDialect::Use(a)).new();
    let dg = stage
        .digraph()
        .node(s_src)
        .node(consumer)
        .yield_value(a)
        .new();

    // `%b` is the replacement value (a live block argument).
    let block = stage.block().argument(TestType::I32).new();

    let mut stage = stage.finalize().unwrap();
    let real_b = SSAValue::from(block.expect_info(&stage).arguments[0]);

    // Before: a is used by the operand and the yield; b is unused.
    assert_eq!(
        uses_set(&stage, a),
        HashSet::from([so(consumer, 0), dgy(dg, 0)])
    );
    assert!(uses_set(&stage, real_b).is_empty());

    let events = {
        let mut rewriter = Rewriter::new(&mut stage);
        let replaced = rewriter.replace_all_uses(a, real_b).unwrap();
        assert_eq!(replaced, 2, "one operand slot + one yield slot");
        rewriter.drain_events()
    };

    // After: a has no uses; b absorbed both (operand + yield).
    assert!(uses_set(&stage, a).is_empty());
    assert_eq!(
        uses_set(&stage, real_b),
        HashSet::from([so(consumer, 0), dgy(dg, 0)])
    );

    // The slots themselves now read b.
    match consumer.definition(&stage) {
        BuilderDialect::Use(v) => assert_eq!(*v, real_b),
        other => panic!("expected Use, got {other:?}"),
    }
    assert_eq!(dg.expect_info(&stage).yields()[0], real_b);

    // Statement operand gets a granular event; the yield is covered by the
    // summary ReplacedUses.
    assert_eq!(
        events,
        vec![
            MutationEvent::ChangedOperands { stmt: consumer },
            MutationEvent::ReplacedUses {
                old: a,
                new: real_b
            },
        ]
    );

    // Maintained index equals a from-scratch rebuild.
    let (ma, mb) = (uses_set(&stage, a), uses_set(&stage, real_b));
    stage.rebuild_use_index();
    assert_eq!(uses_set(&stage, a), ma);
    assert_eq!(uses_set(&stage, real_b), mb);
}

#[test]
fn erase_statement_unlinks_and_maintains_index() {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let s0 = stage.statement().definition(BuilderDialect::Use(x)).new();
    let s1 = stage.statement().definition(BuilderDialect::Use(x)).new();
    let s2 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .stmt(s0)
        .stmt(s1)
        .stmt(s2)
        .new();

    let mut stage = stage.finalize().unwrap();
    let real_x = SSAValue::from(block.expect_info(&stage).arguments[0]);

    assert_eq!(
        uses_set(&stage, real_x),
        HashSet::from([so(s0, 0), so(s1, 0)])
    );

    {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_statement(s1).unwrap();
        assert_eq!(
            rw.drain_events(),
            vec![MutationEvent::ErasedStatement { stmt: s1 }]
        );
    }

    // s1 is tombstoned; the block now reads [s0, s2]; x lost the {s1,0} use.
    assert!(s1.get_info(&stage).unwrap().deleted());
    let order: Vec<Statement> = block.statements(&stage).collect();
    assert_eq!(order, vec![s0, s2]);
    assert_eq!(uses_set(&stage, real_x), HashSet::from([so(s0, 0)]));

    // Maintained index equals a from-scratch rebuild.
    let m = uses_set(&stage, real_x);
    stage.rebuild_use_index();
    assert_eq!(uses_set(&stage, real_x), m);
}

#[test]
fn erase_statement_rejects_terminator_results_and_unknown() {
    let mut stage = new_stage();

    let (wire, wire_ssa) = make_wire(&mut stage);
    let consumer = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let _block = stage
        .block()
        .stmt(wire)
        .stmt(consumer)
        .stmt(s0)
        .terminator(ret)
        .new();

    let mut stage = stage.finalize().unwrap();
    let mut rw = Rewriter::new(&mut stage);

    // A statement whose result is still used cannot be erased.
    assert_eq!(
        rw.erase_statement(wire),
        Err(RewriteError::StatementResultsInUse(wire))
    );
    // The block terminator cannot be erased in this slice.
    assert_eq!(
        rw.erase_statement(ret),
        Err(RewriteError::CannotEraseTerminator(ret))
    );
    // A clean erase succeeds; erasing the same id again reports it unknown.
    rw.erase_statement(s0).unwrap();
    assert_eq!(
        rw.erase_statement(s0),
        Err(RewriteError::UnknownStatement(s0))
    );

    // The rejected edits left wire and consumer intact.
    assert!(!wire.get_info(rw.stage()).unwrap().deleted());
    assert!(!consumer.get_info(rw.stage()).unwrap().deleted());
}

#[test]
fn insert_before_and_after_splice_and_maintain_index() {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let s0 = stage.statement().definition(BuilderDialect::Use(x)).new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(s0)
        .terminator(ret)
        .new();

    let mut stage = stage.finalize().unwrap();
    let real_x = SSAValue::from(block.expect_info(&stage).arguments[0]);
    let real_y = SSAValue::from(block.expect_info(&stage).arguments[1]);

    let (n0, n1, events) = {
        let mut rw = Rewriter::new(&mut stage);
        let n0 = rw.insert_before(s0, BuilderDialect::Use(real_y)).unwrap();
        let n1 = rw.insert_after(s0, BuilderDialect::Use(real_x)).unwrap();
        (n0, n1, rw.drain_events())
    };

    let order: Vec<Statement> = block.statements(&stage).collect();
    assert_eq!(order, vec![n0, s0, n1]);
    assert_eq!(
        events,
        vec![
            MutationEvent::InsertedStatement { stmt: n0 },
            MutationEvent::InsertedStatement { stmt: n1 },
        ]
    );
    // Operand uses for the inserted statements are recorded.
    assert_eq!(uses_set(&stage, real_y), HashSet::from([so(n0, 0)]));
    assert_eq!(
        uses_set(&stage, real_x),
        HashSet::from([so(s0, 0), so(n1, 0)])
    );
    // The terminator is unchanged.
    assert_eq!(block.terminator(&stage), Some(ret));

    // Rejections, none of which mutate the stage.
    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.insert_after(ret, BuilderDialect::Nop),
        Err(RewriteError::AnchorIsTerminator(ret))
    );
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Wire(ResultValue::from(TestSSAValue(0)))),
        Err(RewriteError::CannotInsertWithResults)
    );
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Return),
        Err(RewriteError::CannotInsertTerminator)
    );
    let bogus = SSAValue::from(TestSSAValue(9999));
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Use(bogus)),
        Err(RewriteError::UnknownValue(bogus))
    );
    assert!(rw.events().is_empty());

    // Maintained index equals a from-scratch rebuild.
    let (mx, my) = (uses_set(&stage, real_x), uses_set(&stage, real_y));
    stage.rebuild_use_index();
    assert_eq!(uses_set(&stage, real_x), mx);
    assert_eq!(uses_set(&stage, real_y), my);
}

#[test]
fn replace_statement_swaps_def_and_updates_uses() {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let s0 = stage
        .statement()
        .definition(BuilderDialect::Add(x, x))
        .new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(s0)
        .new();

    let mut stage = stage.finalize().unwrap();
    let real_x = SSAValue::from(block.expect_info(&stage).arguments[0]);
    let real_y = SSAValue::from(block.expect_info(&stage).arguments[1]);

    assert_eq!(
        uses_set(&stage, real_x),
        HashSet::from([so(s0, 0), so(s0, 1)])
    );
    assert!(uses_set(&stage, real_y).is_empty());

    {
        let mut rw = Rewriter::new(&mut stage);
        rw.replace_statement(s0, BuilderDialect::Add(real_y, real_x))
            .unwrap();
        assert_eq!(
            rw.drain_events(),
            vec![MutationEvent::ReplacedStatement { stmt: s0 }]
        );
    }
    match s0.definition(&stage) {
        BuilderDialect::Add(a, b) => {
            assert_eq!(*a, real_y);
            assert_eq!(*b, real_x);
        }
        other => panic!("expected Add, got {other:?}"),
    }
    assert_eq!(uses_set(&stage, real_x), HashSet::from([so(s0, 1)]));
    assert_eq!(uses_set(&stage, real_y), HashSet::from([so(s0, 0)]));

    // Operand-arity change (2 -> 1) with unchanged (zero) result arity.
    {
        let mut rw = Rewriter::new(&mut stage);
        rw.replace_statement(s0, BuilderDialect::Use(real_x))
            .unwrap();
    }
    assert_eq!(uses_set(&stage, real_x), HashSet::from([so(s0, 0)]));
    assert!(uses_set(&stage, real_y).is_empty());

    // Rejections.
    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.replace_statement(s0, BuilderDialect::Wire(ResultValue::from(TestSSAValue(0)))),
        Err(RewriteError::ResultArityMismatch {
            stmt: s0,
            expected: 0,
            found: 1,
        })
    );
    assert_eq!(
        rw.replace_statement(s0, BuilderDialect::Return),
        Err(RewriteError::TerminatorKindMismatch(s0))
    );

    let (mx, my) = (uses_set(&stage, real_x), uses_set(&stage, real_y));
    stage.rebuild_use_index();
    assert_eq!(uses_set(&stage, real_x), mx);
    assert_eq!(uses_set(&stage, real_y), my);
}

#[test]
fn replace_statement_preserves_result_identity() {
    let mut stage = new_stage();

    let (wire, wire_ssa) = make_wire(&mut stage);
    let consumer = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();
    let _block = stage.block().stmt(wire).stmt(consumer).new();

    let mut stage = stage.finalize().unwrap();
    assert_eq!(uses_set(&stage, wire_ssa), HashSet::from([so(consumer, 0)]));

    {
        let mut rw = Rewriter::new(&mut stage);
        // Pass a bogus result id; replace_statement must keep the original.
        rw.replace_statement(
            wire,
            BuilderDialect::Wire(ResultValue::from(TestSSAValue(0))),
        )
        .unwrap();
    }

    // The wire's result slot still names the original SSA value, and the
    // consumer still uses it.
    match wire.definition(&stage) {
        BuilderDialect::Wire(r) => assert_eq!(SSAValue::from(*r), wire_ssa),
        other => panic!("expected Wire, got {other:?}"),
    }
    assert_eq!(uses_set(&stage, wire_ssa), HashSet::from([so(consumer, 0)]));
}
