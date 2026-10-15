//! Integration tests for the Rewriter mutation layer: operand rewriting and
//! mutation-event recording.

mod common;

use std::collections::HashSet;

use common::{BuilderDialect, TestType, make_split, make_wire, new_stage};
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
            SSAValue::from(info.arguments()[0]),
            SSAValue::from(info.arguments()[1]),
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
fn replace_operand_updates_single_slot_and_rejects_illegal_edits() {
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
            SSAValue::from(info.arguments()[0]),
            SSAValue::from(info.arguments()[1]),
        )
    };

    let mut rewriter = Rewriter::new(&mut stage);

    // Rewrite only the second operand; the first must stay untouched.
    let old = rewriter.replace_operand(add, 1, real_y).unwrap();
    assert_eq!(old, real_x);

    // Out-of-range operand index is rejected.
    assert_eq!(
        rewriter.replace_operand(add, 2, real_y),
        Err(RewriteError::OperandIndexOutOfRange {
            stmt: add,
            index: 2
        })
    );

    // A value id that does not resolve to a live SSA value is rejected.
    let bogus = SSAValue::from(TestSSAValue(9999));
    assert_eq!(
        rewriter.replace_operand(add, 0, bogus),
        Err(RewriteError::UnknownValue(bogus))
    );

    // Writing the value already in the slot is a no-op and records no event.
    assert_eq!(rewriter.replace_operand(add, 1, real_y).unwrap(), real_y);

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
fn replace_operand_maintains_def_use_index() {
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
            SSAValue::from(info.arguments()[0]),
            SSAValue::from(info.arguments()[1]),
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
        rewriter.replace_operand(add, 1, real_y).unwrap();
    }

    // After: the {add, 1} use moved from x to y; {add, 0} still reads x.
    assert_eq!(uses_set(&stage, real_x), HashSet::from([so(add, 0)]));
    assert_eq!(uses_set(&stage, real_y), HashSet::from([so(add, 1)]));
    // use-def is untouched — replace_operand changes who reads a value, not its def.
    assert_eq!(*real_x.get_info(&stage).unwrap().kind(), x_kind);
    assert_eq!(*real_y.get_info(&stage).unwrap().kind(), y_kind);

    // The incrementally maintained index equals a from-scratch rebuild.
    verify_derived(&stage).unwrap();
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
    let real_b = SSAValue::from(block.expect_info(&stage).arguments()[0]);

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
    verify_derived(&stage).unwrap();
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
    let real_x = SSAValue::from(block.expect_info(&stage).arguments()[0]);

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
    assert!(s1.get_info(&stage).is_none());
    let order: Vec<Statement> = block.statements(&stage).collect();
    assert_eq!(order, vec![s0, s2]);
    assert_eq!(uses_set(&stage, real_x), HashSet::from([so(s0, 0)]));

    // Maintained index equals a from-scratch rebuild.
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_statement_tombstones_nested_bodies() {
    let mut stage = new_stage();

    // `owner` holds an inner CFG whose statement reads x, a result defined in
    // the outer block.
    let (split, x, _unused) = make_split(&mut stage);
    let inner_use = stage.statement().definition(BuilderDialect::Use(x)).new();
    let inner_ret = stage.statement().definition(BuilderDialect::Return).new();
    let inner_block = stage.block().stmt(inner_use).terminator(inner_ret).new();
    let inner = stage.cfg().add_block(inner_block).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(inner))
        .new();
    let outer = stage.block().stmt(split).stmt(owner).new();

    let mut stage = stage.finalize().unwrap();
    assert_eq!(uses_set(&stage, x), HashSet::from([so(inner_use, 0)]));

    {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_statement(owner).unwrap();
        assert_eq!(
            rw.drain_events(),
            vec![MutationEvent::ErasedStatement { stmt: owner }]
        );
    }

    // Nothing survives under the deleted owner, including the terminator.
    assert!(owner.get_info(&stage).is_none());
    assert!(inner.get_info(&stage).is_none());
    assert!(inner_block.get_info(&stage).is_none());
    assert!(inner_use.get_info(&stage).is_none());
    assert!(inner_ret.get_info(&stage).is_none());
    // The nested statement's use of x went with it; x itself is untouched.
    assert!(uses_set(&stage, x).is_empty());
    assert!(split.get_info(&stage).is_some());
    assert_eq!(outer.statements(&stage).collect::<Vec<_>>(), vec![split]);

    verify_derived(&stage).unwrap();
}

#[test]
fn erase_statement_erases_a_terminator_and_its_edges() {
    let mut stage = new_stage();

    // entry(a) { nop; jump target(a) } ; target(t) { return }
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let target = stage.block().argument(TestType::I32).terminator(ret).new();
    let nop = stage.statement().definition(BuilderDialect::Nop).new();
    let a = stage.block_argument().index(0);
    let jump = stage
        .statement()
        .definition(BuilderDialect::Jump(Successor::from_block(target), vec![a]))
        .new();
    let entry = stage
        .block()
        .argument(TestType::I32)
        .stmt(nop)
        .terminator(jump)
        .new();
    let _cfg = stage.cfg().add_block(entry).add_block(target).new();

    let mut stage = stage.finalize().unwrap();
    let real_a = SSAValue::from(entry.expect_info(&stage).arguments()[0]);
    assert_eq!(uses_set(&stage, real_a), HashSet::from([so(jump, 0)]));
    assert_eq!(target.expect_info(&stage).predecessors(), [entry]);

    let events = {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_statement(jump).unwrap();
        rw.drain_events()
    };
    assert_eq!(
        events,
        vec![
            MutationEvent::ErasedStatement { stmt: jump },
            MutationEvent::ChangedTerminator { block: entry },
        ]
    );

    // The block is unterminated, its chain intact; the edge and use are gone.
    assert!(jump.get_info(&stage).is_none());
    assert_eq!(entry.terminator(&stage), None);
    assert_eq!(entry.statements(&stage).collect::<Vec<_>>(), vec![nop]);
    assert!(target.expect_info(&stage).predecessors().is_empty());
    assert!(uses_set(&stage, real_a).is_empty());
    verify_derived(&stage).unwrap();

    // The block can be closed again.
    Rewriter::new(&mut stage)
        .insert_terminator(entry, BuilderDialect::Return)
        .unwrap();
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_statement_rejects_used_results_and_unknown() {
    let mut stage = new_stage();

    let (wire, wire_ssa) = make_wire(&mut stage);
    let consumer = stage
        .statement()
        .definition(BuilderDialect::Use(wire_ssa))
        .new();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let _block = stage.block().stmt(wire).stmt(consumer).stmt(s0).new();

    let mut stage = stage.finalize().unwrap();
    let mut rw = Rewriter::new(&mut stage);

    // A statement whose result is still used cannot be erased.
    assert_eq!(
        rw.erase_statement(wire),
        Err(RewriteError::StatementResultsInUse(wire))
    );
    // A clean erase succeeds; erasing the same id again reports it unknown.
    rw.erase_statement(s0).unwrap();
    assert_eq!(
        rw.erase_statement(s0),
        Err(RewriteError::UnknownStatement(s0))
    );

    // The rejected edits left wire and consumer intact.
    assert!(wire.get_info(rw.stage()).is_some());
    assert!(consumer.get_info(rw.stage()).is_some());
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
    let real_x = SSAValue::from(block.expect_info(&stage).arguments()[0]);
    let real_y = SSAValue::from(block.expect_info(&stage).arguments()[1]);

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
    let not_fresh = ResultValue::from(TestSSAValue(0));
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Wire(not_fresh)),
        Err(RewriteError::ResultNotFresh(not_fresh.into()))
    );
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Return),
        Err(RewriteError::CannotInsertTerminator)
    );
    // Only a terminator may carry successors.
    assert_eq!(
        rw.insert_after(s0, BuilderDialect::Fork(Successor::from_block(block))),
        Err(RewriteError::NonTerminatorWithSuccessors)
    );
    let bogus = SSAValue::from(TestSSAValue(9999));
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Use(bogus)),
        Err(RewriteError::UnknownValue(bogus))
    );
    assert!(rw.events().is_empty());

    // Maintained index equals a from-scratch rebuild.
    verify_derived(&stage).unwrap();
}

#[test]
fn insert_binds_fresh_results() {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let s0 = stage.statement().definition(BuilderDialect::Use(x)).new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .stmt(s0)
        .terminator(ret)
        .new();
    let mut stage = stage.finalize().unwrap();
    let real_x = SSAValue::from(block.expect_info(&stage).arguments()[0]);

    // A value allocated by a rewriter that never binds it stays dead, and no
    // later rewriter can bind it.
    let stale = Rewriter::new(&mut stage).new_result(TestType::I32);
    assert!(stale.get_info(&stage).is_none());

    let mut rw = Rewriter::new(&mut stage);
    let a = rw.new_result(TestType::I32);
    let b = rw.new_result(TestType::I64);

    // Unbound values are not live yet, and not visible to verification.
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Use(a.into())),
        Err(RewriteError::UnknownValue(a.into()))
    );
    verify_derived(rw.stage()).unwrap();

    // Rejected: the same value twice, an existing value, a stale value.
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Split(a, a)),
        Err(RewriteError::ResultNotFresh(a.into()))
    );
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Wire(ResultValue::from(real_x))),
        Err(RewriteError::ResultNotFresh(real_x))
    );
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Wire(stale)),
        Err(RewriteError::ResultNotFresh(stale.into()))
    );
    // A rejection after the freshness check leaves `a` unbound: the `Split`
    // below still binds it.
    let dead = SSAValue::from(TestSSAValue(9_999));
    assert_eq!(
        rw.insert_before(s0, BuilderDialect::Compute(dead, a)),
        Err(RewriteError::UnknownValue(dead))
    );
    assert!(a.get_info(rw.stage()).is_none());
    assert!(rw.events().is_empty());

    // Binding makes them results of the new statement, in slot order.
    let split = rw.insert_before(s0, BuilderDialect::Split(a, b)).unwrap();
    let info = a.get_info(rw.stage()).unwrap();
    assert_eq!(*info.kind(), SSAKind::Result(split, 0));
    assert_eq!(*info.ty(), TestType::I32);
    let info = b.get_info(rw.stage()).unwrap();
    assert_eq!(*info.kind(), SSAKind::Result(split, 1));
    assert_eq!(*info.ty(), TestType::I64);

    // Bound values are usable, and cannot be bound again.
    let user = rw
        .insert_after(split, BuilderDialect::Use(a.into()))
        .unwrap();
    assert_eq!(
        rw.insert_after(split, BuilderDialect::Wire(b)),
        Err(RewriteError::ResultNotFresh(b.into()))
    );

    // `insert_after` binds results too.
    let c = rw.new_result(TestType::I32);
    let wire = rw.insert_after(split, BuilderDialect::Wire(c)).unwrap();
    let info = c.get_info(rw.stage()).unwrap();
    assert_eq!(*info.kind(), SSAKind::Result(wire, 0));
    assert_eq!(*info.ty(), TestType::I32);

    // One event per successful insert; the rejections recorded none.
    assert_eq!(
        rw.drain_events(),
        vec![
            MutationEvent::InsertedStatement { stmt: split },
            MutationEvent::InsertedStatement { stmt: user },
            MutationEvent::InsertedStatement { stmt: wire },
        ]
    );
    drop(rw);

    assert_eq!(uses_set(&stage, a), HashSet::from([so(user, 0)]));
    assert_eq!(
        block.statements(&stage).collect::<Vec<_>>(),
        vec![split, wire, user, s0]
    );
    verify_derived(&stage).unwrap();
}

#[test]
fn insert_before_terminator_fills_a_terminator_only_block() {
    let mut stage = new_stage();

    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let block = stage.block().terminator(ret).new();
    let mut stage = stage.finalize().unwrap();

    // With no non-terminator to anchor on, the terminator is the only way in.
    let (n0, events) = {
        let mut rw = Rewriter::new(&mut stage);
        let n0 = rw.insert_before(ret, BuilderDialect::Nop).unwrap();
        (n0, rw.drain_events())
    };

    assert_eq!(events, vec![MutationEvent::InsertedStatement { stmt: n0 }]);
    // The new statement is both ends of the chain: walking from `head` and
    // from `tail` each finds it alone.
    assert_eq!(block.statements(&stage).collect::<Vec<_>>(), vec![n0]);
    assert_eq!(block.statements(&stage).rev().collect::<Vec<_>>(), vec![n0]);
    // The terminator is unchanged and still last.
    assert_eq!(block.terminator(&stage), Some(ret));
    assert_eq!(block.last_statement(&stage), Some(ret));

    // Maintained mirrors equal a from-scratch rebuild.
    verify_derived(&stage).unwrap();
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
    let real_x = SSAValue::from(block.expect_info(&stage).arguments()[0]);
    let real_y = SSAValue::from(block.expect_info(&stage).arguments()[1]);

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

    verify_derived(&stage).unwrap();
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

#[test]
fn replace_statement_rejects_illegal_edits() {
    let mut stage = new_stage();

    // A digraph-owned statement, for the body-kind guard.
    let graph_stmt = stage.statement().definition(BuilderDialect::Nop).new();
    let _digraph = stage.digraph().node(graph_stmt).new();

    // A block holding a plain statement, a two-result statement, and a branch.
    let x = stage.block_argument().index(0);
    let plain = stage.statement().definition(BuilderDialect::Use(x)).new();
    let (split, _r0, _r1) = make_split(&mut stage);
    let target = stage.block().new();
    let branch = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(target)))
        .new();
    let source = stage
        .block()
        .argument(TestType::I32)
        .stmt(plain)
        .stmt(split)
        .terminator(branch)
        .new();
    let _cfg = stage.cfg().add_block(source).add_block(target).new();

    let mut stage = stage.finalize().unwrap();
    let real_x = SSAValue::from(source.expect_info(&stage).arguments()[0]);
    // Ids past the end of their arenas.
    let dead_value = SSAValue::from(Id::from(TestSSAValue(9_999)));
    let dead_block = Block::from(Id::from(TestSSAValue(9_999)));

    let mut rw = Rewriter::new(&mut stage);

    // Graph bodies need topology surgery, which is a later slice.
    assert_eq!(
        rw.replace_statement(graph_stmt, BuilderDialect::Nop),
        Err(RewriteError::NotInBlockBody(graph_stmt))
    );
    // Result arity is fixed: dropping results would orphan their SSA values.
    assert_eq!(
        rw.replace_statement(split, BuilderDialect::Nop),
        Err(RewriteError::ResultArityMismatch {
            stmt: split,
            expected: 2,
            found: 0,
        })
    );
    // Terminator-ness is fixed in both directions, or the block's terminator
    // cache would desync.
    assert_eq!(
        rw.replace_statement(plain, BuilderDialect::Return),
        Err(RewriteError::TerminatorKindMismatch(plain))
    );
    assert_eq!(
        rw.replace_statement(branch, BuilderDialect::Nop),
        Err(RewriteError::TerminatorKindMismatch(branch))
    );
    // A non-terminator cannot gain successors: only the terminator's edges
    // are in the predecessor mirror.
    assert_eq!(
        rw.replace_statement(plain, BuilderDialect::Fork(Successor::from_block(target))),
        Err(RewriteError::NonTerminatorWithSuccessors)
    );
    // Operands must be live...
    assert_eq!(
        rw.replace_statement(plain, BuilderDialect::Use(dead_value)),
        Err(RewriteError::UnknownValue(dead_value))
    );
    // ...and so must successor targets, or the new edge would dangle.
    assert_eq!(
        rw.replace_statement(
            branch,
            BuilderDialect::Branch(Successor::from_block(dead_block))
        ),
        Err(RewriteError::UnknownBlock(dead_block))
    );

    // None of the rejections above mutated the stage
    assert_eq!(uses_set(rw.stage(), real_x), HashSet::from([so(plain, 0)]));
    assert_eq!(target.expect_info(rw.stage()).predecessors(), [source]);
    match branch.definition(rw.stage()) {
        BuilderDialect::Branch(successor) => assert_eq!(successor.target(), target),
        other => panic!("expected Branch, got {other:?}"),
    }
}

#[test]
fn replace_statement_maintains_predecessors_when_retargeting() {
    let mut stage = new_stage();

    let old_target = stage.block().new();
    let new_target = stage.block().new();
    let branch = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(old_target)))
        .new();
    let source = stage.block().terminator(branch).new();
    let _cfg = stage
        .cfg()
        .add_block(source)
        .add_block(old_target)
        .add_block(new_target)
        .new();

    let mut stage = stage.finalize().unwrap();
    assert_eq!(old_target.expect_info(&stage).predecessors(), [source]);

    let events = {
        let mut rw = Rewriter::new(&mut stage);
        rw.replace_statement(
            branch,
            BuilderDialect::Branch(Successor::from_block(new_target)),
        )
        .unwrap();
        rw.drain_events()
    };

    // The reverse control-flow index followed the edge.
    assert!(old_target.expect_info(&stage).predecessors().is_empty());
    assert_eq!(new_target.expect_info(&stage).predecessors(), [source]);
    assert_eq!(
        events,
        vec![
            MutationEvent::ReplacedStatement { stmt: branch },
            MutationEvent::ChangedTerminator { block: source },
        ]
    );

    // The maintained index equals a from-scratch rebuild.
    verify_derived(&stage).unwrap();
}

/// The CSE shape `replace_results` exists for: redirect a duplicate's whole
/// result list at the original's, then erase the duplicate.
#[test]
fn replace_results_redirects_every_result_and_unblocks_erase() {
    let mut stage = new_stage();

    let (orig, orig_a, orig_b) = make_split(&mut stage);
    let (dup, dup_a, dup_b) = make_split(&mut stage);
    // One consumer reads both duplicate results; another reads just the second.
    let both = stage
        .statement()
        .definition(BuilderDialect::Add(dup_a, dup_b))
        .new();
    let second = stage
        .statement()
        .definition(BuilderDialect::Use(dup_b))
        .new();
    let _block = stage
        .block()
        .stmt(orig)
        .stmt(dup)
        .stmt(both)
        .stmt(second)
        .new();

    let mut stage = stage.finalize().unwrap();
    assert_eq!(uses_set(&stage, dup_a), HashSet::from([so(both, 0)]));
    assert_eq!(
        uses_set(&stage, dup_b),
        HashSet::from([so(both, 1), so(second, 0)])
    );

    let events = {
        let mut rw = Rewriter::new(&mut stage);
        rw.replace_results(dup, &[orig_a, orig_b]).unwrap();
        rw.drain_events()
    };

    // Both results moved to the originals; the duplicate is now unread.
    assert!(uses_set(&stage, dup_a).is_empty());
    assert!(uses_set(&stage, dup_b).is_empty());
    assert_eq!(uses_set(&stage, orig_a), HashSet::from([so(both, 0)]));
    assert_eq!(
        uses_set(&stage, orig_b),
        HashSet::from([so(both, 1), so(second, 0)])
    );
    match both.definition(&stage) {
        BuilderDialect::Add(a, b) => {
            assert_eq!(*a, orig_a);
            assert_eq!(*b, orig_b);
        }
        other => panic!("expected Add, got {other:?}"),
    }

    // `both` reads two redirected results but is named once.
    assert_eq!(
        events,
        vec![
            MutationEvent::ChangedOperands { stmt: both },
            MutationEvent::ChangedOperands { stmt: second },
            MutationEvent::ReplacedUses {
                old: dup_a,
                new: orig_a,
            },
            MutationEvent::ReplacedUses {
                old: dup_b,
                new: orig_b,
            },
        ],
        "one ChangedOperands per affected statement, then one ReplacedUses per result"
    );

    // The point of the redirection: erasing the duplicate is now legal.
    {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_statement(dup).unwrap();
    }
    assert!(dup.get_info(&stage).is_none());

    verify_derived(&stage).unwrap();
}

/// Substitution is simultaneous, so naming one of the statement's own results
/// as a replacement redirects the two independently. A sequential loop of
/// `replace_all_uses` would instead let the second pair carry away the sites
/// the first one just moved.
#[test]
fn replace_results_substitutes_simultaneously() {
    let mut stage = new_stage();

    let (split, a, b) = make_split(&mut stage);
    let (other, x, _y) = make_split(&mut stage);
    let reads_a = stage.statement().definition(BuilderDialect::Use(a)).new();
    let reads_b = stage.statement().definition(BuilderDialect::Use(b)).new();
    let _block = stage
        .block()
        .stmt(split)
        .stmt(other)
        .stmt(reads_a)
        .stmt(reads_b)
        .new();

    let mut stage = stage.finalize().unwrap();

    // a -> b and b -> x at once.
    {
        let mut rw = Rewriter::new(&mut stage);
        rw.replace_results(split, &[b, x]).unwrap();
    }

    // The site that read `a` reads `b`; the site that read `b` reads `x`.
    match reads_a.definition(&stage) {
        BuilderDialect::Use(v) => assert_eq!(*v, b),
        other => panic!("expected Use, got {other:?}"),
    }
    match reads_b.definition(&stage) {
        BuilderDialect::Use(v) => assert_eq!(*v, x),
        other => panic!("expected Use, got {other:?}"),
    }
    // `b` holds exactly the site redirected to it, not that site plus its own.
    assert!(uses_set(&stage, a).is_empty());
    assert_eq!(uses_set(&stage, b), HashSet::from([so(reads_a, 0)]));
    assert_eq!(uses_set(&stage, x), HashSet::from([so(reads_b, 0)]));

    verify_derived(&stage).unwrap();
}

#[test]
fn replace_results_rejects_bad_arity_and_dead_ids() {
    let mut stage = new_stage();

    let (split, a, b) = make_split(&mut stage);
    let nop = stage.statement().definition(BuilderDialect::Nop).new();
    let consumer = stage.statement().definition(BuilderDialect::Use(a)).new();
    let _block = stage.block().stmt(split).stmt(nop).stmt(consumer).new();

    let mut stage = stage.finalize().unwrap();
    let before = (uses_set(&stage, a), uses_set(&stage, b));

    let events = {
        let mut rw = Rewriter::new(&mut stage);

        // Too few, too many, and none-for-two are all arity mismatches.
        for (replacements, found) in [(&[b][..], 1), (&[b, a, b][..], 3), (&[][..], 0)] {
            assert_eq!(
                rw.replace_results(split, replacements),
                Err(RewriteError::ResultArityMismatch {
                    stmt: split,
                    expected: 2,
                    found,
                })
            );
        }

        // A result-less statement takes an empty list and rejects anything else.
        assert_eq!(rw.replace_results(nop, &[]), Ok(()));
        assert_eq!(
            rw.replace_results(nop, &[a]),
            Err(RewriteError::ResultArityMismatch {
                stmt: nop,
                expected: 0,
                found: 1,
            })
        );

        // A dead replacement is rejected before any pair is applied.
        let dead = SSAValue::from(TestSSAValue(999));
        assert_eq!(
            rw.replace_results(split, &[a, dead]),
            Err(RewriteError::UnknownValue(dead))
        );

        // A tombstoned statement is unknown, not a zero-result statement.
        rw.erase_statement(nop).unwrap();
        assert_eq!(
            rw.replace_results(nop, &[]),
            Err(RewriteError::UnknownStatement(nop))
        );

        // Replacing every result by itself is a no-op.
        assert_eq!(rw.replace_results(split, &[a, b]), Ok(()));
        rw.drain_events()
    };

    // Only the erase recorded anything; no rejected call moved a use list.
    assert_eq!(events, vec![MutationEvent::ErasedStatement { stmt: nop }]);
    assert_eq!((uses_set(&stage, a), uses_set(&stage, b)), before);
}
