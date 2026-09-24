//! Integration tests for derived-metadata verification.
//!
//! The bulk of this file is one test per [`Rewriter`] method, each making a
//! change that *should* move a mirror and then asserting the mirror actually
//! moved with it. Methods are covered per mirror they can affect: every
//! mutation touches the use index, but only `replace_statement` can change a
//! control-flow edge, so it is the only one under predecessors.
//!
//! A handful of non-rewrite tests bracket those: one proves finalization
//! populates the mirrors, one proves `verify_derived` actually reports a
//! mismatch. Without the latter every "…keeps the index in step" assertion
//! would still pass against a verifier that returned `Ok(())` unconditionally.
//!
//! The [`VerifyError::Derive`] arm (corrupt authoritative IR) needs to
//! tombstone a value out from under a live operand, which has no public entry
//! point; it belongs in a crate-internal unit test once `derive_mirrors` has a
//! caller inside the crate.

mod common;

use std::collections::HashSet;

use common::{BuilderDialect, TestType, make_split, new_stage};
use kirin_ir::*;

/// The def-use set of `value` (order-agnostic; `SSAInfo::uses` is a bag).
fn uses_of(stage: &StageInfo<BuilderDialect>, value: SSAValue) -> HashSet<Use> {
    value
        .get_info(stage)
        .map(|info| info.uses().iter().copied().collect())
        .unwrap_or_default()
}

fn operand(stmt: Statement, index: usize) -> Use {
    Use::StatementOperand { stmt, index }
}

fn yielded(graph: DiGraph, index: usize) -> Use {
    Use::DiGraphYield { graph, index }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// One block with two arguments: `x` is read by `add` and `consumer`,
/// `y` only by `add`.
struct OperandStage {
    stage: StageInfo<BuilderDialect>,
    block: Block,
    add: Statement,
    consumer: Statement,
}

fn operand_stage() -> OperandStage {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let y = stage.block_argument().index(1);
    let add = stage
        .statement()
        .definition(BuilderDialect::Add(x, y))
        .new();
    let consumer = stage.statement().definition(BuilderDialect::Use(x)).new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(add)
        .stmt(consumer)
        .new();

    OperandStage {
        stage: stage.finalize().unwrap(),
        block,
        add,
        consumer,
    }
}

impl OperandStage {
    fn x(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(&self.stage).arguments[0])
    }

    fn y(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(&self.stage).arguments[1])
    }
}

/// A digraph yielding two values across its body boundary: `a` is also read by
/// a statement operand, `b` is read by the yield alone.
struct DiGraphStage {
    stage: StageInfo<BuilderDialect>,
    digraph: DiGraph,
    a: SSAValue,
    b: SSAValue,
    consumer: Statement,
}

fn digraph_stage() -> DiGraphStage {
    let mut stage = new_stage();

    let a_src = stage.statement().definition(BuilderDialect::Nop).new();
    let a = stage
        .ssa()
        .ty(TestType::I32)
        .kind(BuilderSSAKind::Result(a_src, 0))
        .new();
    let b_src = stage.statement().definition(BuilderDialect::Nop).new();
    let b = stage
        .ssa()
        .ty(TestType::I32)
        .kind(BuilderSSAKind::Result(b_src, 0))
        .new();

    let consumer = stage.statement().definition(BuilderDialect::Use(a)).new();
    let digraph = stage
        .digraph()
        .node(a_src)
        .node(b_src)
        .node(consumer)
        .yield_value(a)
        .yield_value(b)
        .new();

    DiGraphStage {
        stage: stage.finalize().unwrap(),
        digraph,
        a,
        b,
        consumer,
    }
}

/// Two blocks branching to a third.
struct BranchingStage {
    stage: StageInfo<BuilderDialect>,
    source0: Block,
    source1: Block,
    target: Block,
}

fn branching_stage() -> BranchingStage {
    let mut stage = new_stage();

    let target = stage.block().new();
    let branch0 = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(target)))
        .new();
    let branch1 = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(target)))
        .new();
    let source0 = stage.block().terminator(branch0).new();
    let source1 = stage.block().terminator(branch1).new();
    let _cfg = stage
        .cfg()
        .add_block(source0)
        .add_block(source1)
        .add_block(target)
        .new();

    BranchingStage {
        stage: stage.finalize().unwrap(),
        source0,
        source1,
        target,
    }
}

// ---------------------------------------------------------------------------
// Finalization populates the mirrors, and verification has teeth
// ---------------------------------------------------------------------------

#[test]
fn finalize_populates_both_mirrors() {
    let f = operand_stage();
    assert_eq!(
        uses_of(&f.stage, f.x()),
        HashSet::from([operand(f.add, 0), operand(f.consumer, 0)])
    );
    assert_eq!(uses_of(&f.stage, f.y()), HashSet::from([operand(f.add, 1)]));
    assert_eq!(verify_derived(&f.stage), Ok(()));

    let b = branching_stage();
    assert_eq!(
        b.target.expect_info(&b.stage).predecessors.as_slice(),
        [b.source0, b.source1]
    );
    assert!(b.source0.expect_info(&b.stage).predecessors.is_empty());
    assert_eq!(verify_derived(&b.stage), Ok(()));
}

#[test]
fn finalize_counts_digraph_yields_as_uses() {
    let f = digraph_stage();

    // `a` is read twice: once as an operand, once as a boundary yield.
    assert_eq!(
        uses_of(&f.stage, f.a),
        HashSet::from([operand(f.consumer, 0), yielded(f.digraph, 0)])
    );
    // `b` has no statement reading it — the yield is its only use, and an
    // operand-only scan would have missed it entirely.
    assert_eq!(
        uses_of(&f.stage, f.b),
        HashSet::from([yielded(f.digraph, 1)])
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn parallel_edges_contribute_one_predecessor_entry() {
    let mut stage = new_stage();
    let target = stage.block().new();
    let successor = Successor::from_block(target);
    // Both arms of the conditional branch land on the same block.
    let branch = stage
        .statement()
        .definition(BuilderDialect::CondBranch(successor, successor))
        .new();
    let source = stage.block().terminator(branch).new();
    let _cfg = stage.cfg().add_block(source).add_block(target).new();
    let stage = stage.finalize().unwrap();

    assert_eq!(
        target.expect_info(&stage).predecessors.as_slice(),
        [source],
        "two edges between the same pair of blocks are still one predecessor"
    );
    // `multiset_eq` preserves multiplicity, so a duplicate entry on either side
    // would surface here rather than being silently collapsed.
    assert_eq!(verify_derived(&stage), Ok(()));
}

/// Load-bearing: every other test below asserts `Ok(())`, so without this one
/// a `verify_derived` that never reported anything would pass the whole file.
#[test]
fn a_hand_corrupted_mirror_is_reported_as_a_mismatch() {
    let mut f = operand_stage();
    let x = f.x();

    // Corrupt the mirror only — both operand slots still read `x`.
    x.get_info_mut(&mut f.stage).unwrap().uses_mut().clear();

    let Err(VerifyError::Mismatch(mismatches)) = verify_derived(&f.stage) else {
        panic!("expected a mirror mismatch");
    };
    assert_eq!(mismatches.len(), 1);
    let Mismatch::Uses {
        value,
        installed,
        derived,
    } = &mismatches[0]
    else {
        panic!("expected a use-list mismatch, got {:?}", mismatches[0]);
    };
    assert_eq!(*value, x);
    assert!(installed.is_empty());
    assert_eq!(
        derived.iter().copied().collect::<HashSet<_>>(),
        HashSet::from([operand(f.add, 0), operand(f.consumer, 0)])
    );
}

// ---------------------------------------------------------------------------
// Uses mirror: one test per Rewriter method
// ---------------------------------------------------------------------------

#[test]
fn replace_operand_keeps_the_use_index_in_step() {
    let mut f = operand_stage();
    let (x, y) = (f.x(), f.y());

    {
        let mut rewriter = Rewriter::new(&mut f.stage);
        // `add` goes from `add x, y` to `add x, x`.
        assert_eq!(rewriter.replace_operand(f.add, 1, x).unwrap(), y);
    }

    // `x` is now read twice by `add` alone — a duplicate the mirror must keep,
    // since `multiset_eq` compares multiplicity.
    assert_eq!(
        f.x().get_info(&f.stage).unwrap().uses().len(),
        3,
        "two operands of add plus the consumer"
    );
    assert!(uses_of(&f.stage, y).is_empty());
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn replace_all_uses_keeps_the_use_index_in_step() {
    let mut f = digraph_stage();

    {
        let mut rewriter = Rewriter::new(&mut f.stage);
        // One operand slot and one yield slot both move from `a` to `b`.
        assert_eq!(rewriter.replace_all_uses(f.a, f.b).unwrap(), 2);
    }

    assert!(uses_of(&f.stage, f.a).is_empty());
    assert_eq!(
        uses_of(&f.stage, f.b),
        HashSet::from([
            operand(f.consumer, 0),
            yielded(f.digraph, 0),
            yielded(f.digraph, 1),
        ])
    );
    assert_eq!(f.digraph.expect_info(&f.stage).yields()[0], f.b);
    assert_eq!(
        verify_derived(&f.stage),
        Ok(()),
        "the yield half of the index must move too, not just operands"
    );
}

#[test]
fn replace_results_keeps_the_use_index_in_step() {
    let mut stage = new_stage();

    let (split, r0, r1) = make_split(&mut stage);
    let c0 = stage.statement().definition(BuilderDialect::Use(r0)).new();
    let c1 = stage.statement().definition(BuilderDialect::Use(r1)).new();
    // `keep` reads the block arguments, which is what resolves them at
    // finalize and gives each replacement a pre-existing use to be added to.
    let x = stage.block_argument().index(0);
    let y = stage.block_argument().index(1);
    let keep = stage
        .statement()
        .definition(BuilderDialect::Add(x, y))
        .new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(split)
        .stmt(c0)
        .stmt(c1)
        .stmt(keep)
        .new();

    let mut stage = stage.finalize().unwrap();
    let real_x = SSAValue::from(block.expect_info(&stage).arguments[0]);
    let real_y = SSAValue::from(block.expect_info(&stage).arguments[1]);

    {
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter.replace_results(split, &[real_x, real_y]).unwrap();
    }

    // Both results are now unread, and the block arguments absorbed them
    // alongside the uses they already had.
    assert!(uses_of(&stage, r0).is_empty());
    assert!(uses_of(&stage, r1).is_empty());
    assert_eq!(
        uses_of(&stage, real_x),
        HashSet::from([operand(keep, 0), operand(c0, 0)])
    );
    assert_eq!(
        uses_of(&stage, real_y),
        HashSet::from([operand(keep, 1), operand(c1, 0)])
    );
    assert_eq!(verify_derived(&stage), Ok(()));
}

#[test]
fn erase_statement_keeps_the_use_index_in_step() {
    let mut f = operand_stage();
    let x = f.x();

    {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter.erase_statement(f.consumer).unwrap();
    }

    // The erased statement's operand use is gone; `add`'s is untouched.
    assert_eq!(uses_of(&f.stage, x), HashSet::from([operand(f.add, 0)]));
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn insert_before_keeps_the_use_index_in_step() {
    let mut f = operand_stage();
    let y = f.y();

    let inserted = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter
            .insert_before(f.consumer, BuilderDialect::Use(y))
            .unwrap()
    };

    // The spliced statement's operand must be registered as a new use of `y`.
    assert_eq!(
        uses_of(&f.stage, y),
        HashSet::from([operand(f.add, 1), operand(inserted, 0)])
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn insert_after_keeps_the_use_index_in_step() {
    let mut f = operand_stage();
    let y = f.y();

    let inserted = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter
            .insert_after(f.add, BuilderDialect::Use(y))
            .unwrap()
    };

    assert_eq!(
        uses_of(&f.stage, y),
        HashSet::from([operand(f.add, 1), operand(inserted, 0)])
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn replace_statement_keeps_the_use_index_in_step() {
    let mut f = operand_stage();
    let (x, y) = (f.x(), f.y());

    {
        let mut rewriter = Rewriter::new(&mut f.stage);
        // `consumer` goes from reading `x` to reading `y`.
        rewriter
            .replace_statement(f.consumer, BuilderDialect::Use(y))
            .unwrap();
    }

    assert_eq!(uses_of(&f.stage, x), HashSet::from([operand(f.add, 0)]));
    assert_eq!(
        uses_of(&f.stage, y),
        HashSet::from([operand(f.add, 1), operand(f.consumer, 0)])
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

// ---------------------------------------------------------------------------
// Predecessors mirror
//
// `replace_statement` is the only method that can change a control-flow edge.
// The operand-rewriting methods write through `arguments_mut()`, which yields
// `&mut SSAValue` and so cannot reach a `Successor`; `erase_statement` and the
// `insert_*` methods reject terminators outright.
// ---------------------------------------------------------------------------

#[test]
fn replace_statement_keeps_the_predecessor_index_in_step() {
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

    {
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter
            .replace_statement(
                branch,
                BuilderDialect::Branch(Successor::from_block(new_target)),
            )
            .unwrap();
    }

    // The edge moved, and so did the predecessor entry.
    assert!(old_target.expect_info(&stage).predecessors.is_empty());
    assert_eq!(
        new_target.expect_info(&stage).predecessors.as_slice(),
        [source]
    );
    assert_eq!(verify_derived(&stage), Ok(()));
}

/// The case the set-difference logic exists for: an entry is one per
/// `(source, target)` pair, not one per edge, so a target that keeps *any*
/// edge must keep its entry — and must not gain a second one.
#[test]
fn replace_statement_handles_parallel_edges_in_both_directions() {
    let mut stage = new_stage();
    let t = stage.block().new();
    let u = stage.block().new();
    let branch = stage
        .statement()
        .definition(BuilderDialect::CondBranch(
            Successor::from_block(t),
            Successor::from_block(t),
        ))
        .new();
    let source = stage.block().terminator(branch).new();
    let _cfg = stage
        .cfg()
        .add_block(source)
        .add_block(t)
        .add_block(u)
        .new();
    let mut stage = stage.finalize().unwrap();
    assert_eq!(t.expect_info(&stage).predecessors.as_slice(), [source]);

    // Narrowing: `cond_branch(t, t)` -> `cond_branch(t, u)`. One edge to `t`
    // survives, so `t` keeps its single entry; `u` gains one.
    {
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter
            .replace_statement(
                branch,
                BuilderDialect::CondBranch(Successor::from_block(t), Successor::from_block(u)),
            )
            .unwrap();
    }
    assert_eq!(
        t.expect_info(&stage).predecessors.as_slice(),
        [source],
        "an edge to `t` remains, so its entry must survive"
    );
    assert_eq!(u.expect_info(&stage).predecessors.as_slice(), [source]);
    assert_eq!(verify_derived(&stage), Ok(()));

    // Widening back: `cond_branch(t, u)` -> `cond_branch(t, t)`. `t` must not
    // gain a duplicate entry, and `u` must lose its only one.
    {
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter
            .replace_statement(
                branch,
                BuilderDialect::CondBranch(Successor::from_block(t), Successor::from_block(t)),
            )
            .unwrap();
    }
    assert_eq!(
        t.expect_info(&stage).predecessors.as_slice(),
        [source],
        "`t` already had an entry; a second edge must not add another"
    );
    assert!(u.expect_info(&stage).predecessors.is_empty());
    assert_eq!(verify_derived(&stage), Ok(()));
}

#[test]
fn replace_statement_rejects_a_successor_that_is_not_live() {
    let mut stage = new_stage();
    let target = stage.block().new();
    let branch = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(target)))
        .new();
    let source = stage.block().terminator(branch).new();
    let _cfg = stage.cfg().add_block(source).add_block(target).new();
    let mut stage = stage.finalize().unwrap();

    // An id past the end of the block arena, fabricated the way the dialect
    // tests do it.
    let ghost = Block::from(Id::from(TestSSAValue(9_999)));

    let result = {
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter.replace_statement(branch, BuilderDialect::Branch(Successor::from_block(ghost)))
    };
    assert_eq!(result, Err(RewriteError::UnknownBlock(ghost)));

    // Rejected before any write: the original edge is untouched.
    assert_eq!(
        target.expect_info(&stage).predecessors.as_slice(),
        [source],
        "a rejected edit must leave the stage exactly as it was"
    );
    assert_eq!(verify_derived(&stage), Ok(()));
}

// ---------------------------------------------------------------------------
// Block-body mirror: the chain summary and the terminator
// ---------------------------------------------------------------------------

/// A block whose body is a three-statement chain plus a terminator, so the
/// chain and the terminator can move independently of one another.
struct BodyStage {
    stage: StageInfo<BuilderDialect>,
    block: Block,
    first: Statement,
    middle: Statement,
    last: Statement,
    terminator: Statement,
}

fn body_stage() -> BodyStage {
    let mut stage = new_stage();

    let first = stage.statement().definition(BuilderDialect::Nop).new();
    let middle = stage.statement().definition(BuilderDialect::Nop).new();
    let last = stage.statement().definition(BuilderDialect::Nop).new();
    let terminator = stage.statement().definition(BuilderDialect::Return).new();
    let block = stage
        .block()
        .stmt(first)
        .stmt(middle)
        .stmt(last)
        .terminator(terminator)
        .new();

    BodyStage {
        stage: stage.finalize().unwrap(),
        block,
        first,
        middle,
        last,
        terminator,
    }
}

impl BodyStage {
    fn body(
        &self,
    ) -> (
        Option<Statement>,
        Option<Statement>,
        usize,
        Option<Statement>,
    ) {
        let info = self.block.expect_info(&self.stage);
        (
            info.statements.head().copied(),
            info.statements.tail().copied(),
            info.statements.len(),
            info.terminator,
        )
    }
}

#[test]
fn finalize_populates_the_block_body_mirror() {
    let f = body_stage();

    // The terminator is a member of the block but not a link in the chain, so
    // `len` counts three, not four, and `tail` is the last non-terminator.
    assert_eq!(
        f.body(),
        (Some(f.first), Some(f.last), 3, Some(f.terminator))
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn a_hand_corrupted_block_body_is_reported_as_a_mismatch() {
    let mut f = body_stage();

    // Corrupt the mirror only — the `prev`/`next` links still describe the
    // same three-statement chain.
    f.block.expect_info_mut(&mut f.stage).statements = LinkedList::new();

    let Err(VerifyError::Mismatch(mismatches)) = verify_derived(&f.stage) else {
        panic!("expected a mirror mismatch");
    };
    assert_eq!(mismatches.len(), 1);
    let Mismatch::BlockBody {
        block,
        installed,
        derived,
    } = &mismatches[0]
    else {
        panic!("expected a block-body mismatch, got {:?}", mismatches[0]);
    };
    assert_eq!(*block, f.block);
    assert_eq!(installed.statements.len(), 0);
    assert_eq!(derived.statements.len(), 3);
    // The terminator half is untouched and agrees.
    assert_eq!(installed.terminator, derived.terminator);
}

#[test]
fn erase_statement_keeps_the_block_body_in_step() {
    let mut f = body_stage();

    {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter.erase_statement(f.middle).unwrap();
    }

    assert_eq!(
        f.body(),
        (Some(f.first), Some(f.last), 2, Some(f.terminator))
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erasing_the_head_of_the_chain_keeps_the_block_body_in_step() {
    let mut f = body_stage();

    {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter.erase_statement(f.first).unwrap();
    }

    // `head` moves; the terminator is unaffected.
    assert_eq!(
        f.body(),
        (Some(f.middle), Some(f.last), 2, Some(f.terminator))
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erasing_the_tail_of_the_chain_keeps_the_block_body_in_step() {
    let mut f = body_stage();

    {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter.erase_statement(f.last).unwrap();
    }

    // `tail` moves back to the last surviving non-terminator, not to the
    // terminator, which was never in the chain.
    assert_eq!(
        f.body(),
        (Some(f.first), Some(f.middle), 2, Some(f.terminator))
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn insert_before_keeps_the_block_body_in_step() {
    let mut f = body_stage();

    let inserted = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter
            .insert_before(f.first, BuilderDialect::Nop)
            .unwrap()
    };

    assert_eq!(
        f.body(),
        (Some(inserted), Some(f.last), 4, Some(f.terminator))
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn insert_after_keeps_the_block_body_in_step() {
    let mut f = body_stage();

    let inserted = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter.insert_after(f.last, BuilderDialect::Nop).unwrap()
    };

    assert_eq!(
        f.body(),
        (Some(f.first), Some(inserted), 4, Some(f.terminator))
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn a_block_holding_only_a_terminator_has_an_empty_chain() {
    let mut stage = new_stage();
    let terminator = stage.statement().definition(BuilderDialect::Return).new();
    let block = stage.block().terminator(terminator).new();
    let stage = stage.finalize().unwrap();

    // Empty is a chain, not a failure: the mirror derives and installs.
    let info = block.expect_info(&stage);
    assert_eq!(info.statements.head(), None);
    assert_eq!(info.statements.len(), 0);
    assert_eq!(info.terminator, Some(terminator));
    assert_eq!(verify_derived(&stage), Ok(()));
}

// ---------------------------------------------------------------------------
// CFG block-list mirror
//
// `CFGInfo::blocks` is `pub(crate)`, so these read it through the public
// `CFG::blocks()` iterator and cannot corrupt it from out here. There is no
// verifier failure tests for this mirror as a result, nor any public path
// that could desync it since block-list surgery does not exist yet.
// ---------------------------------------------------------------------------

/// A CFG holding three blocks, each with its own terminator.
struct CfgStage {
    stage: StageInfo<BuilderDialect>,
    cfg: CFG,
    first: Block,
    middle: Block,
    last: Block,
}

fn cfg_stage() -> CfgStage {
    let mut stage = new_stage();

    let mut block = || {
        let terminator = stage.statement().definition(BuilderDialect::Return).new();
        stage.block().terminator(terminator).new()
    };
    let (first, middle, last) = (block(), block(), block());
    let cfg = stage
        .cfg()
        .add_block(first)
        .add_block(middle)
        .add_block(last)
        .new();

    CfgStage {
        stage: stage.finalize().unwrap(),
        cfg,
        first,
        middle,
        last,
    }
}

#[test]
fn finalize_populates_the_cfg_block_list_mirror() {
    let f = cfg_stage();

    assert_eq!(
        f.cfg.blocks(&f.stage).collect::<Vec<_>>(),
        vec![f.first, f.middle, f.last]
    );
    // `BlockIter` is double-ended, so walking backwards exercises `tail` and
    // the `prev` links rather than `head`/`next` a second time.
    assert_eq!(
        f.cfg.blocks(&f.stage).rev().collect::<Vec<_>>(),
        vec![f.last, f.middle, f.first]
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn a_cfg_with_one_block_is_its_own_head_and_tail() {
    let mut stage = new_stage();
    let terminator = stage.statement().definition(BuilderDialect::Return).new();
    let only = stage.block().terminator(terminator).new();
    let cfg = stage.cfg().add_block(only).new();
    let stage = stage.finalize().unwrap();

    assert_eq!(cfg.blocks(&stage).collect::<Vec<_>>(), vec![only]);
    assert_eq!(cfg.blocks(&stage).rev().collect::<Vec<_>>(), vec![only]);
    assert_eq!(verify_derived(&stage), Ok(()));
}

#[test]
fn blocks_owned_directly_by_a_statement_join_no_cfg_chain() {
    let mut stage = new_stage();

    // A single-block body hanging off an operation — an `scf.if` arm in a real
    // dialect. Its parent is a statement, not a CFG, so it is a member of no
    // block list and must not be reported as an orphan of one.
    let arm_term = stage.statement().definition(BuilderDialect::Return).new();
    let arm = stage.block().terminator(arm_term).new();
    let other_term = stage.statement().definition(BuilderDialect::Return).new();
    let other = stage.block().terminator(other_term).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(arm, other))
        .new();
    let outer_term = stage.statement().definition(BuilderDialect::Return).new();
    let outer = stage.block().stmt(owner).terminator(outer_term).new();
    let cfg = stage.cfg().add_block(outer).new();
    let stage = stage.finalize().unwrap();

    // The CFG lists only the block actually parented to it.
    assert_eq!(cfg.blocks(&stage).collect::<Vec<_>>(), vec![outer]);
    assert_eq!(verify_derived(&stage), Ok(()));
}
