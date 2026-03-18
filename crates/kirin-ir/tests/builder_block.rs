//! Integration tests for block builder, region builder, statement iteration,
//! detach, SSA creation, and linked list helpers.

mod common;

use common::{BuilderDialect, TestType, new_stage};
use kirin_ir::*;

// --- BlockBuilder tests ---

#[test]
fn block_builder_creates_block_with_arguments_and_statements() {
    let mut stage = new_stage();

    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();

    let block = stage
        .block()
        .argument(TestType::I32)
        .arg_name("x")
        .argument(TestType::I64)
        .arg_name("y")
        .stmt(s0)
        .stmt(s1)
        .new();

    let stage = stage.into_inner();
    let info = block.expect_info(&stage);
    assert_eq!(info.arguments.len(), 2);

    for (idx, &arg) in info.arguments.iter().enumerate() {
        let ssa = arg.expect_info(&stage);
        assert_eq!(*ssa.kind(), SSAKind::BlockArgument(block, idx));
    }

    let stmts: Vec<_> = block.statements(&stage).collect();
    assert_eq!(stmts.len(), 2);
    assert_eq!(stmts[0], s0);
    assert_eq!(stmts[1], s1);
}

#[test]
fn block_builder_substitutes_builder_block_arguments() {
    let mut stage = new_stage();

    let arg0 = stage.block_argument().index(0);
    let arg1 = stage.block_argument().index(1);

    let add_stmt = stage
        .statement()
        .definition(BuilderDialect::Add(arg0.into(), arg1.into()))
        .new();

    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I64)
        .stmt(add_stmt)
        .new();

    let stage = stage.into_inner();
    let block_info = block.expect_info(&stage);
    let real_arg0: SSAValue = block_info.arguments[0].into();
    let real_arg1: SSAValue = block_info.arguments[1].into();

    match add_stmt.definition(&stage) {
        BuilderDialect::Add(a, b) => {
            assert_eq!(*a, real_arg0, "first arg should be substituted");
            assert_eq!(*b, real_arg1, "second arg should be substituted");
        }
        _ => panic!("expected Add"),
    }

    let ssa0 = real_arg0.get_info(&stage).unwrap();
    assert!(matches!(ssa0.kind(), SSAKind::BlockArgument(_, 0)));
    let ssa1 = real_arg1.get_info(&stage).unwrap();
    assert!(matches!(ssa1.kind(), SSAKind::BlockArgument(_, 1)));
}

#[test]
#[should_panic(expected = "is not a terminator")]
fn block_builder_terminator_rejects_non_terminator() {
    let mut stage = new_stage();
    let nop = stage.statement().definition(BuilderDialect::Nop).new();
    let _ = stage.block().terminator(nop).new();
}

#[test]
#[should_panic(expected = "Cannot add terminator statement")]
fn block_builder_stmt_rejects_terminator() {
    let mut stage = new_stage();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let _ = stage.block().stmt(ret).new();
}

// --- StatementIter tests ---

#[test]
fn statement_iter_double_ended() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let s2 = stage.statement().definition(BuilderDialect::Nop).new();

    let block = stage.block().stmt(s0).stmt(s1).stmt(s2).new();

    let stage = stage.into_inner();
    let mut iter = block.statements(&stage);
    let last = iter.next_back().unwrap();
    let mid = iter.next_back().unwrap();
    let first = iter.next_back().unwrap();
    assert_eq!(first, s0);
    assert_eq!(mid, s1);
    assert_eq!(last, s2);
    assert!(iter.next_back().is_none());
}

#[test]
fn statement_iter_exact_size() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();

    let block = stage.block().stmt(s0).stmt(s1).new();

    let stage = stage.into_inner();
    let mut iter = block.statements(&stage);
    assert_eq!(iter.len(), 2);
    iter.next();
    assert_eq!(iter.len(), 1);
    iter.next();
    assert_eq!(iter.len(), 0);
}

#[test]
fn block_first_last_statement_with_terminator_only() {
    let mut stage = new_stage();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let block = stage.block().terminator(ret).new();

    let stage = stage.into_inner();
    assert_eq!(block.statements(&stage).len(), 0);
    assert_eq!(block.first_statement(&stage), Some(ret));
    assert_eq!(block.last_statement(&stage), Some(ret));
}

#[test]
fn block_last_statement_without_terminator() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage.block().stmt(s0).stmt(s1).new();

    let stage = stage.into_inner();
    assert_eq!(block.terminator(&stage), None);
    assert_eq!(block.last_statement(&stage), Some(s1));
    assert_eq!(block.first_statement(&stage), Some(s0));
}

#[test]
fn block_with_statements_and_terminator() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();

    let block = stage.block().stmt(s0).stmt(s1).terminator(ret).new();

    let stage = stage.into_inner();
    let stmts: Vec<_> = block.statements(&stage).collect();
    assert_eq!(stmts, vec![s0, s1]);
    assert_eq!(block.terminator(&stage), Some(ret));
    assert_eq!(block.first_statement(&stage), Some(s0));
    assert_eq!(block.last_statement(&stage), Some(ret));
}

#[test]
fn empty_block_iteration() {
    let mut stage = new_stage();
    let block = stage.block().new();

    let stage = stage.into_inner();
    let stmts: Vec<_> = block.statements(&stage).collect();
    assert!(stmts.is_empty());
    assert_eq!(block.statements(&stage).len(), 0);
    assert_eq!(block.first_statement(&stage), None);
    assert_eq!(block.last_statement(&stage), None);
    assert_eq!(block.terminator(&stage), None);
}

#[test]
fn single_statement_double_ended_iteration() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage.block().stmt(s0).new();

    let stage = stage.into_inner();
    let mut iter = block.statements(&stage);
    assert_eq!(iter.next(), Some(s0));
    assert_eq!(iter.next(), None);

    let mut iter = block.statements(&stage);
    assert_eq!(iter.next_back(), Some(s0));
    assert_eq!(iter.next_back(), None);
}

#[test]
fn block_argument_placeholder_substitution_with_zero_args() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage.block().stmt(s0).new();

    let stage = stage.into_inner();
    let info = block.expect_info(&stage);
    assert!(info.arguments.is_empty());
}

// --- RegionBuilder tests ---

#[test]
fn region_builder_creates_region_with_ordered_blocks() {
    let mut stage = new_stage();
    let b0 = stage.block().new();
    let b1 = stage.block().new();
    let b2 = stage.block().new();

    let region = stage
        .region()
        .add_block(b0)
        .add_block(b1)
        .add_block(b2)
        .new();

    let stage = stage.into_inner();
    assert_eq!(region.blocks(&stage).len(), 3);
    let blocks: Vec<_> = region.blocks(&stage).collect();
    assert_eq!(blocks, vec![b0, b1, b2]);

    let b0_info = b0.expect_info(&stage);
    assert_eq!(b0_info.node.next, Some(b1));
    let b1_info = b1.expect_info(&stage);
    assert_eq!(b1_info.node.prev, Some(b0));
    assert_eq!(b1_info.node.next, Some(b2));
    let b2_info = b2.expect_info(&stage);
    assert_eq!(b2_info.node.prev, Some(b1));
    assert_eq!(b2_info.node.next, None);
}

#[test]
#[should_panic(expected = "already added to the region")]
fn region_builder_panics_on_duplicate_block() {
    let mut stage = new_stage();
    let b0 = stage.block().new();
    let _ = stage.region().add_block(b0).add_block(b0).new();
}

#[test]
fn region_block_iter_single_block() {
    let mut stage = new_stage();
    let b0 = stage.block().new();
    let region = stage.region().add_block(b0).new();

    let stage = stage.into_inner();
    let blocks: Vec<_> = region.blocks(&stage).collect();
    assert_eq!(blocks, vec![b0]);
    assert_eq!(region.blocks(&stage).len(), 1);
}

#[test]
fn region_block_iter_double_ended() {
    let mut stage = new_stage();
    let b0 = stage.block().new();
    let b1 = stage.block().new();
    let b2 = stage.block().new();
    let region = stage
        .region()
        .add_block(b0)
        .add_block(b1)
        .add_block(b2)
        .new();

    let stage = stage.into_inner();
    let mut iter = region.blocks(&stage);
    assert_eq!(iter.next_back(), Some(b2));
    assert_eq!(iter.next(), Some(b0));
    assert_eq!(iter.next_back(), Some(b1));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
}

#[test]
fn region_block_iter_exact_size() {
    let mut stage = new_stage();
    let b0 = stage.block().new();
    let b1 = stage.block().new();
    let region = stage.region().add_block(b0).add_block(b1).new();

    let stage = stage.into_inner();
    let mut iter = region.blocks(&stage);
    assert_eq!(iter.len(), 2);
    iter.next();
    assert_eq!(iter.len(), 1);
    iter.next();
    assert_eq!(iter.len(), 0);
}

#[test]
fn empty_region() {
    let mut stage = new_stage();
    let region = stage.region().new();

    let stage = stage.into_inner();
    let blocks: Vec<_> = region.blocks(&stage).collect();
    assert!(blocks.is_empty());
    assert_eq!(region.blocks(&stage).len(), 0);
}

// --- Detach tests ---

#[test]
fn detach_statement_updates_neighbors_and_parent_len() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let s2 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage.block().stmt(s0).stmt(s1).stmt(s2).new();

    stage.with_inner(|inner| {
        s1.detach(inner);
    });

    let stage = stage.into_inner();
    let block_info = block.expect_info(&stage);
    assert_eq!(block_info.statements.len(), 2);

    assert_eq!(*s0.next(&stage), Some(s2));
    assert_eq!(*s2.prev(&stage), Some(s0));

    assert_eq!(*s1.prev(&stage), None);
    assert_eq!(*s1.next(&stage), None);
    assert_eq!(*s1.parent(&stage), None);
}

#[test]
fn detach_head_statement_updates_block_head() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage.block().stmt(s0).stmt(s1).new();

    stage.with_inner(|inner| {
        s0.detach(inner);
    });

    let stage = stage.into_inner();
    let block_info = block.expect_info(&stage);
    assert_eq!(block_info.statements.head(), Some(&s1));
    assert_eq!(block_info.statements.len(), 1);
}

#[test]
fn detach_tail_statement_updates_block_tail() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let s1 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage.block().stmt(s0).stmt(s1).new();

    stage.with_inner(|inner| {
        s1.detach(inner);
    });

    let stage = stage.into_inner();
    let block_info = block.expect_info(&stage);
    assert_eq!(block_info.statements.tail(), Some(&s0));
    assert_eq!(block_info.statements.len(), 1);
}

#[test]
fn detach_only_statement_leaves_empty_block() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage.block().stmt(s0).new();

    stage.with_inner(|inner| {
        s0.detach(inner);
    });

    let stage = stage.into_inner();
    let block_info = block.expect_info(&stage);
    assert_eq!(block_info.statements.len(), 0);
    assert!(block_info.statements.head().is_none());
    assert!(block_info.statements.tail().is_none());
}

// --- SSA creation edge cases ---

#[test]
fn ssa_with_name_is_resolvable() {
    let mut stage = new_stage();
    let ssa = stage
        .ssa()
        .name("x")
        .ty(TestType::I32)
        .kind(BuilderSSAKind::Test)
        .new();

    let info = stage.ssa_arena().get(ssa).unwrap();
    assert!(info.name().is_some());
    assert_eq!(info.ty(), Some(&TestType::I32));
    assert_eq!(*info.builder_kind(), BuilderSSAKind::Test);
}

#[test]
fn ssa_without_name() {
    let mut stage = new_stage();
    let ssa = stage
        .ssa()
        .ty(TestType::I64)
        .kind(BuilderSSAKind::Test)
        .new();

    let info = stage.ssa_arena().get(ssa).unwrap();
    assert!(info.name().is_none());
    assert_eq!(info.ty(), Some(&TestType::I64));
}

// --- link_statements edge cases ---

#[test]
fn link_statements_empty_slice() {
    let mut stage = new_stage();
    let list = stage.link_statements(&[]);
    assert_eq!(list.len(), 0);
    assert!(list.head().is_none());
    assert!(list.tail().is_none());
}

#[test]
fn link_statements_single_element() {
    let mut stage = new_stage();
    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let list = stage.link_statements(&[s0]);
    assert_eq!(list.len(), 1);
    assert_eq!(list.head(), Some(&s0));
    assert_eq!(list.tail(), Some(&s0));
    let stage = stage.into_inner();
    assert_eq!(*s0.prev(&stage), None);
    assert_eq!(*s0.next(&stage), None);
}

// --- link_blocks edge cases ---

#[test]
fn link_blocks_empty_slice() {
    let mut stage = new_stage();
    let list = stage.link_blocks(&[]);
    assert_eq!(list.len(), 0);
    assert!(list.head().is_none());
    assert!(list.tail().is_none());
}

#[test]
fn link_blocks_single_element() {
    let mut stage = new_stage();
    let b0 = stage.block().new();
    let list = stage.link_blocks(&[b0]);
    assert_eq!(list.len(), 1);
    assert_eq!(list.head(), Some(&b0));
    assert_eq!(list.tail(), Some(&b0));
}

// --- remap_block_identity ---

#[test]
fn remap_block_identity_remaps_parents_and_ssa_kinds() {
    let mut stage = new_stage();

    let stub = stage.block().new();

    let s0 = stage.statement().definition(BuilderDialect::Nop).new();
    let real = stage.block().argument(TestType::I32).stmt(s0).new();

    stage.remap_block_identity(stub, real);

    let stage = stage.into_inner();
    assert_eq!(*s0.parent(&stage), Some(StatementParent::Block(stub)));

    let stub_info = stub.expect_info(&stage);
    assert_eq!(stub_info.arguments.len(), 1);
    let arg = stub_info.arguments[0];
    let arg_info = arg.expect_info(&stage);
    assert!(matches!(*arg_info.kind(), SSAKind::BlockArgument(owner, 0) if owner == stub));

    assert!(stage.block_arena().get(real).unwrap().deleted());
}
