//! Integration tests for the Rewriter's CFG block-list surgery: block
//! creation, erasure, splitting, merging, edge retargeting, and block-argument
//! insertion and erasure.

mod common;

use std::collections::HashSet;

use common::{BuilderDialect, TestType, make_split, new_stage};
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

fn br(target: Block) -> BuilderDialect {
    BuilderDialect::Branch(Successor::from_block(target))
}

/// The `index`-th argument of `block`.
fn arg(stage: &StageInfo<BuilderDialect>, block: Block, index: usize) -> SSAValue {
    block.expect_info(stage).arguments[index].into()
}

fn preds(stage: &StageInfo<BuilderDialect>, block: Block) -> HashSet<Block> {
    block
        .expect_info(stage)
        .predecessors
        .iter()
        .copied()
        .collect()
}

fn ssa_deleted(stage: &StageInfo<BuilderDialect>, value: impl Into<SSAValue>) -> bool {
    stage.ssa_arena().get(value.into()).unwrap().deleted()
}

#[test]
fn append_and_insert_blocks_link_into_cfg() {
    let mut stage = new_stage();

    let entry = stage.block().new();
    let exit = stage.block().new();
    let cfg = stage.cfg().add_block(entry).add_block(exit).new();

    let mut stage = stage.finalize().unwrap();

    let (tail, before, after, events) = {
        let mut rw = Rewriter::new(&mut stage);
        let tail = rw.append_block(cfg, [TestType::I32]).unwrap();
        let before = rw.insert_block_before(entry, []).unwrap();
        let after = rw
            .insert_block_after(entry, [TestType::I64, TestType::I32])
            .unwrap();
        (tail, before, after, rw.drain_events())
    };

    assert_eq!(
        events,
        vec![MutationEvent::ChangedBlockList { cfg }; 3],
        "one ChangedBlockList per created block"
    );

    // The block list reads the same forwards and backwards.
    let order: Vec<Block> = cfg.blocks(&stage).collect();
    assert_eq!(order, vec![before, entry, after, exit, tail]);
    let reversed: Vec<Block> = cfg.blocks(&stage).rev().collect();
    assert_eq!(reversed, vec![tail, exit, after, entry, before]);

    // Fresh arguments carry their declared type and positional kind, in order.
    for (block, expected) in [
        (before, vec![]),
        (after, vec![TestType::I64, TestType::I32]),
        (tail, vec![TestType::I32]),
    ] {
        let args = &block.expect_info(&stage).arguments;
        assert_eq!(args.len(), expected.len(), "argument count of {block}");
        for (index, (&arg, ty)) in args.iter().zip(&expected).enumerate() {
            let info = SSAValue::from(arg).get_info(&stage).unwrap();
            assert_eq!(info.ty(), ty, "type of {block} argument {index}");
            assert_eq!(info.kind(), &SSAKind::BlockArgument(block, index));
        }
    }
    assert!(tail.terminator(&stage).is_none(), "new blocks start empty");

    verify_derived(&stage).unwrap();
}

#[test]
fn append_and_insert_blocks_reject_unknown_and_non_cfg_anchors() {
    let mut stage = new_stage();

    let entry = stage.block().new();
    let _cfg = stage.cfg().add_block(entry).new();
    // A block that belongs to no CFG block list.
    let lone = stage.block().new();

    let mut stage = stage.finalize().unwrap();
    let dead_cfg = CFG::from(Id::from(TestSSAValue(9_999)));
    let dead_block = Block::from(Id::from(TestSSAValue(9_999)));

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.append_block(dead_cfg, []),
        Err(RewriteError::UnknownCfg(dead_cfg))
    );
    assert_eq!(
        rw.insert_block_after(lone, []),
        Err(RewriteError::NotInCfgBody(lone))
    );
    assert_eq!(
        rw.insert_block_before(dead_block, []),
        Err(RewriteError::UnknownBlock(dead_block))
    );
    assert!(rw.events().is_empty(), "rejections record nothing");
}

#[test]
fn erase_block_tombstones_contents_and_maintains_mirrors() {
    let mut stage = new_stage();

    // entry(y) -> exit, and an unreachable `dead` block that reads y and
    // branches to exit.
    let exit_ret = stage.statement().definition(BuilderDialect::Return).new();
    let exit = stage.block().terminator(exit_ret).new();
    let entry_br = stage.statement().definition(br(exit)).new();
    let entry = stage
        .block()
        .argument(TestType::I32)
        .terminator(entry_br)
        .new();
    let x = stage.block_argument().index(0);
    let use_x = stage.statement().definition(BuilderDialect::Use(x)).new();
    let dead_br = stage.statement().definition(br(exit)).new();
    let dead = stage
        .block()
        .argument(TestType::I32)
        .stmt(use_x)
        .terminator(dead_br)
        .new();
    let cfg = stage
        .cfg()
        .add_block(entry)
        .add_block(dead)
        .add_block(exit)
        .new();

    let mut stage = stage.finalize().unwrap();
    let real_x = arg(&stage, dead, 0);
    assert_eq!(preds(&stage, exit), HashSet::from([entry, dead]));

    let events = {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_block(dead).unwrap();
        rw.drain_events()
    };
    assert_eq!(
        events,
        vec![
            MutationEvent::ChangedBlockList { cfg },
            MutationEvent::ChangedCfg { cfg },
        ]
    );

    // The block, its statements, and its argument are tombstoned.
    assert!(dead.get_info(&stage).unwrap().deleted());
    assert!(use_x.get_info(&stage).unwrap().deleted());
    assert!(dead_br.get_info(&stage).unwrap().deleted());
    assert!(ssa_deleted(&stage, real_x));

    // The block list and the reverse control-flow index forgot it.
    let order: Vec<Block> = cfg.blocks(&stage).collect();
    assert_eq!(order, vec![entry, exit]);
    assert_eq!(preds(&stage, exit), HashSet::from([entry]));

    verify_derived(&stage).unwrap();
}

#[test]
fn erase_block_rejects_incoming_edges_until_retargeted() {
    let mut stage = new_stage();

    // entry -> middle -> exit
    let exit_ret = stage.statement().definition(BuilderDialect::Return).new();
    let exit = stage.block().terminator(exit_ret).new();
    let middle_br = stage.statement().definition(br(exit)).new();
    let middle = stage.block().terminator(middle_br).new();
    let entry_br = stage.statement().definition(br(middle)).new();
    let entry = stage.block().terminator(entry_br).new();
    let cfg = stage
        .cfg()
        .add_block(entry)
        .add_block(middle)
        .add_block(exit)
        .new();

    let mut stage = stage.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.erase_block(middle),
        Err(RewriteError::BlockHasPredecessors {
            block: middle,
            pred: entry,
        })
    );
    assert!(rw.events().is_empty());
    assert!(!middle.get_info(rw.stage()).unwrap().deleted());
    assert_eq!(preds(rw.stage(), exit), HashSet::from([middle]));
    verify_derived(rw.stage()).unwrap();

    // Once the edge is retargeted the erase goes through.
    rw.retarget_successor(entry_br, middle, exit).unwrap();
    rw.erase_block(middle).unwrap();
    assert_eq!(
        cfg.blocks(rw.stage()).collect::<Vec<_>>(),
        vec![entry, exit]
    );
    assert_eq!(preds(rw.stage(), exit), HashSet::from([entry]));
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn erase_block_allows_self_loop() {
    let mut stage = new_stage();

    let entry = stage.block().new();
    let lp = stage.block().new();
    let cfg = stage.cfg().add_block(entry).add_block(lp).new();

    let mut stage = stage.finalize().unwrap();
    // The builder cannot name `lp` before creating it, so close the loop here.
    Rewriter::new(&mut stage)
        .insert_terminator(lp, br(lp))
        .unwrap();
    assert_eq!(preds(&stage, lp), HashSet::from([lp]));

    Rewriter::new(&mut stage).erase_block(lp).unwrap();
    assert_eq!(cfg.blocks(&stage).collect::<Vec<_>>(), vec![entry]);
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_block_at_cfg_ends_updates_head_and_tail() {
    let mut stage = new_stage();

    let first = stage.block().new();
    let middle = stage.block().new();
    let last = stage.block().new();
    let cfg = stage
        .cfg()
        .add_block(first)
        .add_block(middle)
        .add_block(last)
        .new();

    let mut stage = stage.finalize().unwrap();

    // Erasing the head makes the next block the head.
    Rewriter::new(&mut stage).erase_block(first).unwrap();
    assert_eq!(cfg.blocks(&stage).collect::<Vec<_>>(), vec![middle, last]);
    verify_derived(&stage).unwrap();

    // Erasing the tail makes the previous block the tail.
    Rewriter::new(&mut stage).erase_block(last).unwrap();
    assert_eq!(cfg.blocks(&stage).collect::<Vec<_>>(), vec![middle]);
    assert_eq!(cfg.blocks(&stage).rev().collect::<Vec<_>>(), vec![middle]);
    verify_derived(&stage).unwrap();
}

#[test]
fn erase_block_rejects_values_used_outside_without_mutating() {
    let mut stage = new_stage();

    // middle(y) { split -> r0 } -> exit { use r0; use_arg }
    let (split, r0, _r1) = make_split(&mut stage);
    let use_r0 = stage.statement().definition(BuilderDialect::Use(r0)).new();
    let exit_ret = stage.statement().definition(BuilderDialect::Return).new();
    let exit = stage.block().stmt(use_r0).terminator(exit_ret).new();
    let middle_br = stage.statement().definition(br(exit)).new();
    let middle = stage
        .block()
        .argument(TestType::I32)
        .stmt(split)
        .terminator(middle_br)
        .new();
    let _cfg = stage.cfg().add_block(middle).add_block(exit).new();

    let mut stage = stage.finalize().unwrap();
    let y = arg(&stage, middle, 0);

    let mut rw = Rewriter::new(&mut stage);
    // A result read by a statement in another block.
    assert_eq!(
        rw.erase_block(middle),
        Err(RewriteError::StatementResultsInUse(split))
    );

    // An argument read outside the block is rejected the same way.
    rw.replace_operand(use_r0, 0, y).unwrap();
    rw.drain_events();
    assert_eq!(
        rw.erase_block(middle),
        Err(RewriteError::BlockArgumentInUse {
            block: middle,
            index: 0,
        })
    );

    // Nothing was touched.
    assert!(rw.events().is_empty());
    assert!(!middle.get_info(rw.stage()).unwrap().deleted());
    assert!(!split.get_info(rw.stage()).unwrap().deleted());
    assert_eq!(preds(rw.stage(), exit), HashSet::from([middle]));
    assert_eq!(uses_set(rw.stage(), y), HashSet::from([so(use_r0, 0)]));
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn erase_block_allows_uses_nested_inside_it() {
    let mut stage = new_stage();

    // host { split -> r0; owner { inner { use r0 } } }: r0's only use is
    // nested in a body inside host, so it is not an outside use.
    let (split, r0, _r1) = make_split(&mut stage);
    let inner_use = stage.statement().definition(BuilderDialect::Use(r0)).new();
    let inner_block = stage.block().stmt(inner_use).new();
    let inner = stage.cfg().add_block(inner_block).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(inner))
        .new();
    let host = stage.block().stmt(split).stmt(owner).new();
    let entry = stage.block().new();
    let cfg = stage.cfg().add_block(entry).add_block(host).new();

    let mut stage = stage.finalize().unwrap();
    Rewriter::new(&mut stage).erase_block(host).unwrap();

    assert!(inner_use.get_info(&stage).unwrap().deleted());
    assert!(split.get_info(&stage).unwrap().deleted());
    assert!(ssa_deleted(&stage, r0));
    assert_eq!(cfg.blocks(&stage).collect::<Vec<_>>(), vec![entry]);

    verify_derived(&stage).unwrap();
}

#[test]
fn erase_block_tombstones_nested_bodies() {
    let mut stage = new_stage();

    // `host` holds a statement that owns an inner CFG with one block.
    let inner_nop = stage.statement().definition(BuilderDialect::Nop).new();
    let inner_ret = stage.statement().definition(BuilderDialect::Return).new();
    let inner_block = stage.block().stmt(inner_nop).terminator(inner_ret).new();
    let inner = stage.cfg().add_block(inner_block).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(inner))
        .new();
    let host_ret = stage.statement().definition(BuilderDialect::Return).new();
    let host = stage.block().stmt(owner).terminator(host_ret).new();
    let entry_ret = stage.statement().definition(BuilderDialect::Return).new();
    let entry = stage.block().terminator(entry_ret).new();
    let _cfg = stage.cfg().add_block(entry).add_block(host).new();

    let mut stage = stage.finalize().unwrap();
    Rewriter::new(&mut stage).erase_block(host).unwrap();

    // Everything reachable from the erased block is tombstoned, not just
    // its direct statements.
    assert!(owner.get_info(&stage).unwrap().deleted());
    assert!(inner.get_info(&stage).unwrap().deleted());
    assert!(inner_block.get_info(&stage).unwrap().deleted());
    assert!(inner_nop.get_info(&stage).unwrap().deleted());
    assert!(inner_ret.get_info(&stage).unwrap().deleted());
    assert!(!entry.get_info(&stage).unwrap().deleted());

    verify_derived(&stage).unwrap();
}

#[test]
fn split_block_moves_suffix_and_leaves_original_unterminated() {
    let mut stage = new_stage();

    let exit_ret = stage.statement().definition(BuilderDialect::Return).new();
    let exit = stage.block().terminator(exit_ret).new();
    let x = stage.block_argument().index(0);
    let s0 = stage.statement().definition(BuilderDialect::Use(x)).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let s2 = stage.statement().definition(BuilderDialect::Use(x)).new();
    let entry_br = stage.statement().definition(br(exit)).new();
    let entry = stage
        .block()
        .argument(TestType::I32)
        .stmt(s0)
        .stmt(s1)
        .stmt(s2)
        .terminator(entry_br)
        .new();
    let cfg = stage.cfg().add_block(entry).add_block(exit).new();

    let mut stage = stage.finalize().unwrap();
    let real_x = arg(&stage, entry, 0);

    let (tail, events) = {
        let mut rw = Rewriter::new(&mut stage);
        let tail = rw.split_block(s1).unwrap();
        (tail, rw.drain_events())
    };
    assert_eq!(
        events,
        vec![
            MutationEvent::ChangedBlockList { cfg },
            MutationEvent::ChangedCfg { cfg },
        ]
    );

    // entry keeps the prefix and has no terminator; the new block holds the
    // suffix and the original terminator.
    assert_eq!(entry.statements(&stage).collect::<Vec<_>>(), vec![s0]);
    assert_eq!(entry.terminator(&stage), None);
    assert_eq!(tail.statements(&stage).collect::<Vec<_>>(), vec![s1, s2]);
    assert_eq!(tail.terminator(&stage), Some(entry_br));
    for stmt in [s1, s2, entry_br] {
        assert_eq!(stmt.parent(&stage), &Some(StatementParent::Block(tail)));
    }
    assert!(tail.expect_info(&stage).arguments.is_empty());
    assert_eq!(
        cfg.blocks(&stage).collect::<Vec<_>>(),
        vec![entry, tail, exit]
    );

    // The moved edge leaves from the new block; nothing reaches it yet.
    assert!(preds(&stage, tail).is_empty());
    assert_eq!(preds(&stage, exit), HashSet::from([tail]));
    assert_eq!(
        uses_set(&stage, real_x),
        HashSet::from([so(s0, 0), so(s2, 0)])
    );
    verify_derived(&stage).unwrap();

    // Closing the first half with a branch, as a caller would, wires the edge.
    Rewriter::new(&mut stage)
        .insert_terminator(entry, br(tail))
        .unwrap();
    assert_eq!(preds(&stage, tail), HashSet::from([entry]));
    verify_derived(&stage).unwrap();
}

#[test]
fn split_block_at_terminator_moves_only_the_terminator() {
    let mut stage = new_stage();

    let exit_ret = stage.statement().definition(BuilderDialect::Return).new();
    let exit = stage.block().terminator(exit_ret).new();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let entry_br = stage.statement().definition(br(exit)).new();
    let entry = stage.block().stmt(s0).terminator(entry_br).new();
    let _cfg = stage.cfg().add_block(entry).add_block(exit).new();

    let mut stage = stage.finalize().unwrap();
    let tail = Rewriter::new(&mut stage).split_block(entry_br).unwrap();

    assert_eq!(entry.statements(&stage).collect::<Vec<_>>(), vec![s0]);
    assert_eq!(entry.terminator(&stage), None);
    assert_eq!(tail.statements(&stage).count(), 0);
    assert_eq!(tail.terminator(&stage), Some(entry_br));
    assert_eq!(preds(&stage, exit), HashSet::from([tail]));

    verify_derived(&stage).unwrap();
}

#[test]
fn split_block_at_first_statement_empties_original_chain() {
    let mut stage = new_stage();

    let exit_ret = stage.statement().definition(BuilderDialect::Return).new();
    let exit = stage.block().terminator(exit_ret).new();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let entry_br = stage.statement().definition(br(exit)).new();
    let entry = stage.block().stmt(s0).stmt(s1).terminator(entry_br).new();
    let _cfg = stage.cfg().add_block(entry).add_block(exit).new();

    let mut stage = stage.finalize().unwrap();
    let tail = Rewriter::new(&mut stage).split_block(s0).unwrap();

    // The whole chain and the terminator moved; entry is empty.
    assert_eq!(entry.statements(&stage).count(), 0);
    assert_eq!(entry.terminator(&stage), None);
    assert_eq!(tail.statements(&stage).collect::<Vec<_>>(), vec![s0, s1]);
    assert_eq!(tail.terminator(&stage), Some(entry_br));
    assert_eq!(s0.prev(&stage), &None, "the moved head has no predecessor");

    verify_derived(&stage).unwrap();
}

#[test]
fn split_block_rejects_unknown_and_non_cfg_statements() {
    let mut stage = new_stage();

    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let entry = stage.block().stmt(s0).terminator(ret).new();
    let cfg = stage.cfg().add_block(entry).new();
    let lone_nop = stage.statement().definition(BuilderDialect::Nop).new();
    let lone = stage.block().stmt(lone_nop).new();

    let mut stage = stage.finalize().unwrap();
    let dead_stmt = Statement::from(Id::from(TestSSAValue(9_999)));
    let blocks_before = stage.block_arena().len();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.split_block(lone_nop),
        Err(RewriteError::NotInCfgBody(lone))
    );
    assert_eq!(
        rw.split_block(dead_stmt),
        Err(RewriteError::UnknownStatement(dead_stmt))
    );
    assert!(rw.events().is_empty());

    // No block was allocated and the original block is intact.
    assert_eq!(rw.stage().block_arena().len(), blocks_before);
    assert_eq!(cfg.blocks(rw.stage()).collect::<Vec<_>>(), vec![entry]);
    assert_eq!(entry.terminator(rw.stage()), Some(ret));
}

#[test]
fn insert_terminator_wires_operands_and_edges() {
    let mut stage = new_stage();

    // entry(a) has no terminator; target(t) returns.
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let target = stage.block().argument(TestType::I32).terminator(ret).new();
    let entry = stage.block().argument(TestType::I32).new();
    let _cfg = stage.cfg().add_block(entry).add_block(target).new();

    let mut stage = stage.finalize().unwrap();
    let a = arg(&stage, entry, 0);

    let (jump, events) = {
        let mut rw = Rewriter::new(&mut stage);
        let jump = rw
            .insert_terminator(
                entry,
                BuilderDialect::Jump(Successor::from_block(target), vec![a]),
            )
            .unwrap();
        (jump, rw.drain_events())
    };
    assert_eq!(
        events,
        vec![
            MutationEvent::InsertedStatement { stmt: jump },
            MutationEvent::ChangedTerminator { block: entry },
        ]
    );

    assert_eq!(entry.terminator(&stage), Some(jump));
    assert_eq!(jump.parent(&stage), &Some(StatementParent::Block(entry)));
    assert_eq!(entry.statements(&stage).count(), 0, "not part of the chain");
    assert_eq!(uses_set(&stage, a), HashSet::from([so(jump, 0)]));
    assert_eq!(preds(&stage, target), HashSet::from([entry]));

    verify_derived(&stage).unwrap();
}

#[test]
fn insert_terminator_works_in_statement_owned_body() {
    let mut stage = new_stage();

    // `owner` owns two single-block bodies, neither terminated.
    let body = stage.block().new();
    let other = stage.block().new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(body, other))
        .new();
    let host_ret = stage.statement().definition(BuilderDialect::Return).new();
    let host = stage.block().stmt(owner).terminator(host_ret).new();
    let _cfg = stage.cfg().add_block(host).new();

    let mut stage = stage.finalize().unwrap();
    let ret = Rewriter::new(&mut stage)
        .insert_terminator(body, BuilderDialect::Return)
        .unwrap();

    assert_eq!(body.terminator(&stage), Some(ret));
    verify_derived(&stage).unwrap();
}

#[test]
fn insert_terminator_rejects_without_mutating() {
    let mut stage = new_stage();

    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let terminated = stage.block().terminator(ret).new();
    let open = stage.block().new();
    let _cfg = stage.cfg().add_block(terminated).add_block(open).new();

    let mut stage = stage.finalize().unwrap();
    let dead_block = Block::from(Id::from(TestSSAValue(9_999)));
    let dead_value = SSAValue::from(TestSSAValue(9_999));
    let stmts_before = stage.statement_arena().len();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.insert_terminator(terminated, BuilderDialect::Return),
        Err(RewriteError::BlockHasTerminator(terminated))
    );
    assert_eq!(
        rw.insert_terminator(open, BuilderDialect::Nop),
        Err(RewriteError::ExpectedTerminator)
    );
    assert_eq!(
        rw.insert_terminator(open, br(dead_block)),
        Err(RewriteError::UnknownBlock(dead_block))
    );
    assert_eq!(
        rw.insert_terminator(
            open,
            BuilderDialect::Jump(Successor::from_block(terminated), vec![dead_value]),
        ),
        Err(RewriteError::UnknownValue(dead_value))
    );
    assert_eq!(
        rw.insert_terminator(dead_block, BuilderDialect::Return),
        Err(RewriteError::UnknownBlock(dead_block))
    );

    assert!(rw.events().is_empty());
    assert_eq!(rw.stage().statement_arena().len(), stmts_before);
    assert_eq!(open.terminator(rw.stage()), None);
    assert!(preds(rw.stage(), terminated).is_empty());
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn merge_blocks_splices_successor_and_redirects_arguments() {
    let mut stage = new_stage();

    // entry(a) -jump(a)-> succ(x) { use x; br exit } ; exit
    let exit_ret = stage.statement().definition(BuilderDialect::Return).new();
    let exit = stage.block().terminator(exit_ret).new();
    let x = stage.block_argument().index(0);
    let use_x = stage.statement().definition(BuilderDialect::Use(x)).new();
    let succ_br = stage.statement().definition(br(exit)).new();
    let succ = stage
        .block()
        .argument(TestType::I32)
        .stmt(use_x)
        .terminator(succ_br)
        .new();
    let a = stage.block_argument().index(0);
    let nop = stage.statement().definition(BuilderDialect::Nop).new();
    let jump = stage
        .statement()
        .definition(BuilderDialect::Jump(Successor::from_block(succ), vec![a]))
        .new();
    let entry = stage
        .block()
        .argument(TestType::I32)
        .stmt(nop)
        .terminator(jump)
        .new();
    let cfg = stage
        .cfg()
        .add_block(entry)
        .add_block(succ)
        .add_block(exit)
        .new();

    let mut stage = stage.finalize().unwrap();
    let (real_a, real_x) = (arg(&stage, entry, 0), arg(&stage, succ, 0));

    let events = {
        let mut rw = Rewriter::new(&mut stage);
        rw.merge_blocks(entry, succ, &[real_a]).unwrap();
        rw.drain_events()
    };
    assert_eq!(
        events,
        vec![
            MutationEvent::ErasedStatement { stmt: jump },
            MutationEvent::ChangedTerminator { block: entry },
            MutationEvent::ChangedOperands { stmt: use_x },
            MutationEvent::ReplacedUses {
                old: real_x,
                new: real_a,
            },
            MutationEvent::ChangedBlockList { cfg },
            MutationEvent::ChangedCfg { cfg },
        ]
    );

    // entry now holds both bodies and succ's terminator.
    assert_eq!(
        entry.statements(&stage).collect::<Vec<_>>(),
        vec![nop, use_x]
    );
    assert_eq!(entry.terminator(&stage), Some(succ_br));
    assert_eq!(use_x.parent(&stage), &Some(StatementParent::Block(entry)));
    assert_eq!(use_x.definition(&stage), &BuilderDialect::Use(real_a));

    // succ, its argument, and entry's old terminator are gone.
    assert!(succ.get_info(&stage).unwrap().deleted());
    assert!(jump.get_info(&stage).unwrap().deleted());
    assert!(ssa_deleted(&stage, real_x));
    assert_eq!(cfg.blocks(&stage).collect::<Vec<_>>(), vec![entry, exit]);

    // a lost the jump's use and gained use_x's; exit's predecessor followed.
    assert_eq!(uses_set(&stage, real_a), HashSet::from([so(use_x, 0)]));
    assert_eq!(preds(&stage, exit), HashSet::from([entry]));

    verify_derived(&stage).unwrap();
}

#[test]
fn merge_blocks_with_empty_pred_chain_moves_succ_chain_to_head() {
    let mut stage = new_stage();

    // entry { br succ } ; succ { s0; s1; return }
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let succ = stage.block().stmt(s0).stmt(s1).terminator(ret).new();
    let entry_br = stage.statement().definition(br(succ)).new();
    let entry = stage.block().terminator(entry_br).new();
    let _cfg = stage.cfg().add_block(entry).add_block(succ).new();

    let mut stage = stage.finalize().unwrap();
    Rewriter::new(&mut stage)
        .merge_blocks(entry, succ, &[])
        .unwrap();

    assert_eq!(entry.statements(&stage).collect::<Vec<_>>(), vec![s0, s1]);
    assert_eq!(entry.terminator(&stage), Some(ret));
    verify_derived(&stage).unwrap();
}

#[test]
fn merge_blocks_with_empty_succ_chain_keeps_pred_chain() {
    let mut stage = new_stage();

    // entry { s0; br succ } ; succ { return }
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let succ = stage.block().terminator(ret).new();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let entry_br = stage.statement().definition(br(succ)).new();
    let entry = stage.block().stmt(s0).terminator(entry_br).new();
    let _cfg = stage.cfg().add_block(entry).add_block(succ).new();

    let mut stage = stage.finalize().unwrap();
    Rewriter::new(&mut stage)
        .merge_blocks(entry, succ, &[])
        .unwrap();

    assert_eq!(entry.statements(&stage).collect::<Vec<_>>(), vec![s0]);
    assert_eq!(entry.terminator(&stage), Some(ret));
    verify_derived(&stage).unwrap();
}

#[test]
fn merge_blocks_rejects_replacements_deleted_with_succ() {
    let mut stage = new_stage();

    // entry(a) { br succ } ; succ(x, y) { use x; return }
    let x = stage.block_argument().index(0);
    let use_x = stage.statement().definition(BuilderDialect::Use(x)).new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let succ = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(use_x)
        .terminator(ret)
        .new();
    let entry_br = stage.statement().definition(br(succ)).new();
    let entry = stage
        .block()
        .argument(TestType::I32)
        .terminator(entry_br)
        .new();
    let _cfg = stage.cfg().add_block(entry).add_block(succ).new();

    let mut stage = stage.finalize().unwrap();
    let a = arg(&stage, entry, 0);
    let (real_x, real_y) = (arg(&stage, succ, 0), arg(&stage, succ, 1));

    let mut rw = Rewriter::new(&mut stage);
    // x replaced by itself: the redirect is a no-op, then x would be deleted.
    assert_eq!(
        rw.merge_blocks(entry, succ, &[real_x, real_y]),
        Err(RewriteError::ReplacementIsMergedArgument {
            block: succ,
            value: real_x,
        })
    );
    // x replaced by succ's other argument, which the merge also deletes.
    assert_eq!(
        rw.merge_blocks(entry, succ, &[real_y, real_y]),
        Err(RewriteError::ReplacementIsMergedArgument {
            block: succ,
            value: real_y,
        })
    );

    // Nothing was touched.
    assert!(rw.events().is_empty());
    assert!(!succ.get_info(rw.stage()).unwrap().deleted());
    assert_eq!(entry.terminator(rw.stage()), Some(entry_br));
    assert_eq!(use_x.definition(rw.stage()), &BuilderDialect::Use(real_x));
    verify_derived(rw.stage()).unwrap();

    // Values that survive the merge are accepted.
    rw.merge_blocks(entry, succ, &[a, a]).unwrap();
    assert_eq!(use_x.definition(rw.stage()), &BuilderDialect::Use(a));
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn merge_blocks_rejects_illegal_merges() {
    let mut stage = new_stage();

    let succ = stage.block().argument(TestType::I32).new();
    let pred = stage.block().new();
    let cfg = stage.cfg().add_block(pred).add_block(succ).new();
    let other = stage.block().new();
    let _other_cfg = stage.cfg().add_block(other).new();
    let lone = stage.block().new();

    let mut stage = stage.finalize().unwrap();
    let real_arg = arg(&stage, succ, 0);

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.merge_blocks(pred, succ, &[]),
        Err(RewriteError::BlockArgumentArityMismatch {
            block: succ,
            expected: 1,
            found: 0,
        })
    );
    assert_eq!(
        rw.merge_blocks(pred, pred, &[]),
        Err(RewriteError::CannotMergeBlockIntoItself(pred))
    );
    assert_eq!(
        rw.merge_blocks(pred, other, &[]),
        Err(RewriteError::CfgMismatch {
            block: other,
            expected: cfg,
        })
    );
    assert_eq!(
        rw.merge_blocks(lone, succ, &[real_arg]),
        Err(RewriteError::NotInCfgBody(lone))
    );
    assert!(rw.events().is_empty());
    assert!(!succ.get_info(rw.stage()).unwrap().deleted());
}

#[test]
fn merge_blocks_rejects_self_looping_succ() {
    let mut stage = new_stage();

    // entry -> succ, succ -> succ
    let succ = stage.block().new();
    let entry_br = stage.statement().definition(br(succ)).new();
    let entry = stage.block().terminator(entry_br).new();
    let _cfg = stage.cfg().add_block(entry).add_block(succ).new();

    let mut stage = stage.finalize().unwrap();
    // The builder cannot name `succ` before creating it, so close the loop here.
    Rewriter::new(&mut stage)
        .insert_terminator(succ, br(succ))
        .unwrap();
    let succ_br = succ.terminator(&stage).unwrap();

    // Merging would move `br succ` into entry, leaving it targeting the
    // tombstoned succ.
    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.merge_blocks(entry, succ, &[]),
        Err(RewriteError::BlockHasPredecessors {
            block: succ,
            pred: succ,
        })
    );
    assert!(rw.events().is_empty());
    assert!(!succ.get_info(rw.stage()).unwrap().deleted());
    assert_eq!(entry.terminator(rw.stage()), Some(entry_br));
    assert_eq!(succ.terminator(rw.stage()), Some(succ_br));
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn merge_blocks_rejects_other_incoming_edges_until_retargeted() {
    let mut stage = new_stage();

    // entry -> succ, side -> succ
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let succ = stage.block().terminator(ret).new();
    let side_br = stage.statement().definition(br(succ)).new();
    let side = stage.block().terminator(side_br).new();
    let entry_br = stage.statement().definition(br(succ)).new();
    let entry = stage.block().terminator(entry_br).new();
    let cfg = stage
        .cfg()
        .add_block(entry)
        .add_block(side)
        .add_block(succ)
        .new();

    let mut stage = stage.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.merge_blocks(entry, succ, &[]),
        Err(RewriteError::BlockHasPredecessors {
            block: succ,
            pred: side,
        })
    );
    assert!(rw.events().is_empty());
    assert!(!succ.get_info(rw.stage()).unwrap().deleted());
    assert_eq!(entry.terminator(rw.stage()), Some(entry_br));
    verify_derived(rw.stage()).unwrap();

    // Once side branches elsewhere the merge goes through.
    rw.retarget_successor(side_br, succ, entry).unwrap();
    rw.merge_blocks(entry, succ, &[]).unwrap();
    assert_eq!(
        cfg.blocks(rw.stage()).collect::<Vec<_>>(),
        vec![entry, side]
    );
    assert_eq!(entry.terminator(rw.stage()), Some(ret));
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn retarget_successor_rewrites_every_matching_slot() {
    let mut stage = new_stage();

    let t = stage.block().new();
    let u = stage.block().new();
    let cond = stage
        .statement()
        .definition(BuilderDialect::CondBranch(
            Successor::from_block(t),
            Successor::from_block(t),
        ))
        .new();
    let entry = stage.block().terminator(cond).new();
    let cfg = stage.cfg().add_block(entry).add_block(t).add_block(u).new();

    let mut stage = stage.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(rw.retarget_successor(cond, t, u).unwrap(), 2);
    assert_eq!(rw.drain_events(), vec![MutationEvent::ChangedCfg { cfg }]);

    // Nothing left to retarget: no-op, no event.
    assert_eq!(rw.retarget_successor(cond, t, u).unwrap(), 0);
    assert!(rw.events().is_empty());

    assert_eq!(
        cond.definition(rw.stage()),
        &BuilderDialect::CondBranch(Successor::from_block(u), Successor::from_block(u))
    );
    assert!(preds(rw.stage(), t).is_empty());
    assert_eq!(preds(rw.stage(), u), HashSet::from([entry]));

    verify_derived(rw.stage()).unwrap();
}

#[test]
fn retarget_successor_rejects_dead_new_target() {
    let mut stage = new_stage();

    let exit = stage.block().new();
    let entry_br = stage.statement().definition(br(exit)).new();
    let entry = stage.block().terminator(entry_br).new();
    let _cfg = stage.cfg().add_block(entry).add_block(exit).new();

    let mut stage = stage.finalize().unwrap();
    let dead_block = Block::from(Id::from(TestSSAValue(9_999)));

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.retarget_successor(entry_br, exit, dead_block),
        Err(RewriteError::UnknownBlock(dead_block))
    );
    assert!(rw.events().is_empty());
    assert_eq!(preds(rw.stage(), exit), HashSet::from([entry]));
    verify_derived(rw.stage()).unwrap();
}

/// Build `entry(a: i32, b: i64) -jump(entry args at forward)-> target(target_args)
/// { use target_args[used] }`, returning `(stage, entry, target, jump, use_stmt)`.
fn jump_into_target(
    forward: &[usize],
    target_args: &[TestType],
    used: usize,
) -> (
    StageInfo<BuilderDialect>,
    Block,
    Block,
    Statement,
    Statement,
) {
    let mut stage = new_stage();

    let used_arg = stage.block_argument().index(used);
    let use_stmt = stage
        .statement()
        .definition(BuilderDialect::Use(used_arg))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let mut target = stage.block();
    for ty in target_args {
        target = target.argument(ty.clone());
    }
    let target = target.stmt(use_stmt).terminator(ret).new();
    // Only forwarded arguments get placeholders: an unreferenced placeholder
    // would never resolve and finalize would reject it.
    let forwarded = forward
        .iter()
        .map(|&index| stage.block_argument().index(index))
        .collect();
    let jump = stage
        .statement()
        .definition(BuilderDialect::Jump(
            Successor::from_block(target),
            forwarded,
        ))
        .new();
    let entry = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I64)
        .terminator(jump)
        .new();
    let _cfg = stage.cfg().add_block(entry).add_block(target).new();

    (stage.finalize().unwrap(), entry, target, jump, use_stmt)
}

fn jump_args(stage: &StageInfo<BuilderDialect>, jump: Statement) -> Vec<SSAValue> {
    match jump.definition(stage) {
        BuilderDialect::Jump(_, args) => args.clone(),
        other => panic!("expected Jump, got {other:?}"),
    }
}

#[test]
fn insert_block_arg_shifts_arguments_and_rewrites_edges() {
    // entry(a, b) -jump(a)-> target(x) { use x }
    let (mut stage, entry, target, jump, use_x) = jump_into_target(&[0], &[TestType::I32], 0);
    let cfg = match target.expect_info(&stage).parent {
        Some(BlockParent::CFG(cfg)) => cfg,
        other => panic!("expected a CFG parent, got {other:?}"),
    };
    let (real_a, real_b) = (arg(&stage, entry, 0), arg(&stage, entry, 1));
    let real_x = arg(&stage, target, 0);

    // Prepend an i64 argument fed by b; the edge must now forward (b, a).
    let (fresh, events) = {
        let mut rw = Rewriter::new(&mut stage);
        let mut asked = Vec::new();
        let fresh = rw
            .insert_block_arg(target, 0, TestType::I64, |stmt, successor| {
                asked.push((stmt, successor));
                real_b
            })
            .unwrap();
        assert_eq!(asked, vec![(jump, 0)], "one value per incoming edge");
        (SSAValue::from(fresh), rw.drain_events())
    };
    assert_eq!(
        events,
        vec![
            MutationEvent::ReplacedStatement { stmt: jump },
            MutationEvent::ChangedTerminator { block: entry },
            MutationEvent::ChangedCfg { cfg },
        ]
    );

    // The fresh argument is first; x keeps its identity and uses but moves to 1.
    assert_eq!(
        target.expect_info(&stage).arguments,
        vec![BlockArgument::from(fresh), BlockArgument::from(real_x)]
    );
    let fresh_info = fresh.get_info(&stage).unwrap();
    assert_eq!(fresh_info.ty(), &TestType::I64);
    assert_eq!(fresh_info.kind(), &SSAKind::BlockArgument(target, 0));
    assert_eq!(
        real_x.get_info(&stage).unwrap().kind(),
        &SSAKind::BlockArgument(target, 1)
    );
    assert_eq!(uses_set(&stage, real_x), HashSet::from([so(use_x, 0)]));

    // The edge forwards the new list, and the use index followed.
    assert_eq!(jump_args(&stage, jump), vec![real_b, real_a]);
    assert_eq!(uses_set(&stage, real_b), HashSet::from([so(jump, 0)]));
    assert_eq!(uses_set(&stage, real_a), HashSet::from([so(jump, 1)]));

    verify_derived(&stage).unwrap();
}

#[test]
fn insert_block_arg_rejects_without_mutating() {
    let (mut stage, _entry, target, jump, _use_x) = jump_into_target(&[0], &[TestType::I32], 0);
    let args_before = target.expect_info(&stage).arguments.clone();
    let ssas_before = stage.ssa_arena().len();
    let dead_value = SSAValue::from(TestSSAValue(9_999));

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.insert_block_arg(target, 2, TestType::I32, |_, _| dead_value),
        Err(RewriteError::BlockArgumentIndexOutOfRange {
            block: target,
            index: 2,
        })
    );
    // A rewritten edge is checked like replace_statement before any edit.
    assert_eq!(
        rw.insert_block_arg(target, 1, TestType::I32, |_, _| dead_value),
        Err(RewriteError::UnknownValue(dead_value))
    );

    assert!(rw.events().is_empty());
    assert_eq!(target.expect_info(rw.stage()).arguments, args_before);
    assert_eq!(
        rw.stage().ssa_arena().len(),
        ssas_before,
        "no value allocated"
    );
    assert_eq!(jump_args(rw.stage(), jump).len(), 1);
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn erase_block_arg_shifts_arguments_and_rewrites_edges() {
    // entry(a, b) -jump(a, b)-> target(x, y) { use y }
    let (mut stage, entry, target, jump, use_y) =
        jump_into_target(&[0, 1], &[TestType::I32, TestType::I64], 1);
    let (real_a, real_b) = (arg(&stage, entry, 0), arg(&stage, entry, 1));
    let (real_x, real_y) = (arg(&stage, target, 0), arg(&stage, target, 1));

    // Drop the unused x; the edge must stop forwarding a.
    {
        let mut rw = Rewriter::new(&mut stage);
        rw.erase_block_arg(target, 0).unwrap();
    }

    assert_eq!(
        target.expect_info(&stage).arguments,
        vec![BlockArgument::from(real_y)]
    );
    assert!(ssa_deleted(&stage, real_x));
    assert_eq!(
        real_y.get_info(&stage).unwrap().kind(),
        &SSAKind::BlockArgument(target, 0)
    );
    assert_eq!(uses_set(&stage, real_y), HashSet::from([so(use_y, 0)]));

    assert_eq!(jump_args(&stage, jump), vec![real_b]);
    assert!(uses_set(&stage, real_a).is_empty());
    assert_eq!(uses_set(&stage, real_b), HashSet::from([so(jump, 0)]));

    verify_derived(&stage).unwrap();
}

#[test]
fn erase_block_arg_rejects_used_and_out_of_range_arguments() {
    let (mut stage, _entry, target, _jump, _use_x) = jump_into_target(&[0], &[TestType::I32], 0);
    let args_before = target.expect_info(&stage).arguments.clone();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        rw.erase_block_arg(target, 0),
        Err(RewriteError::BlockArgumentInUse {
            block: target,
            index: 0,
        })
    );
    assert_eq!(
        rw.erase_block_arg(target, 1),
        Err(RewriteError::BlockArgumentIndexOutOfRange {
            block: target,
            index: 1,
        })
    );

    assert!(rw.events().is_empty());
    assert_eq!(target.expect_info(rw.stage()).arguments, args_before);
}

#[test]
fn block_arg_edits_reject_edges_without_successor_operands() {
    let mut stage = new_stage();

    // entry -branch-> target(x): `Branch` forwards no operand list, so its
    // edge cannot gain or lose an operand.
    let target = stage.block().argument(TestType::I32).new();
    let branch = stage.statement().definition(br(target)).new();
    let entry = stage.block().terminator(branch).new();
    let _cfg = stage.cfg().add_block(entry).add_block(target).new();

    let mut stage = stage.finalize().unwrap();
    let args_before = target.expect_info(&stage).arguments.clone();
    let x = arg(&stage, target, 0);

    let mut rw = Rewriter::new(&mut stage);
    let expected = Err(RewriteError::NoSuccessorOperands {
        stmt: branch,
        successor: 0,
    });
    assert_eq!(rw.erase_block_arg(target, 0), expected);
    assert_eq!(
        rw.insert_block_arg(target, 0, TestType::I32, |_, _| x)
            .map(|_| ()),
        expected
    );

    assert!(rw.events().is_empty());
    assert_eq!(target.expect_info(rw.stage()).arguments, args_before);
    verify_derived(rw.stage()).unwrap();
}
