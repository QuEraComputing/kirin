//! Cloning statements and CFGs with the Rewriter.

mod common;

use std::collections::HashSet;

use common::{BuilderDialect, TestType, make_compute, make_split, new_stage};
use kirin_ir::*;

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

fn order(stage: &StageInfo<BuilderDialect>, block: Block) -> Vec<Statement> {
    block.statements(stage).collect()
}

fn preds(stage: &StageInfo<BuilderDialect>, block: Block) -> HashSet<Block> {
    block
        .expect_info(stage)
        .predecessors()
        .iter()
        .copied()
        .collect()
}

fn definition(stage: &StageInfo<BuilderDialect>, stmt: Statement) -> &BuilderDialect {
    stmt.expect_info(stage).definition()
}

/// ```text
/// entry:
///   %x, _ = split
///   owner = own_blocks { then: %t = compute %x; use %t; ret } { else: use %x; ret }
///   ret
/// ```
struct IfLike {
    stage: StageInfo<BuilderDialect>,
    entry: Block,
    x: SSAValue,
    then_block: Block,
    t: SSAValue,
    compute: Statement,
    use_t: Statement,
    owner: Statement,
}

fn if_like() -> IfLike {
    let mut stage = new_stage();
    let (split, x, _) = make_split(&mut stage);
    let (compute, t) = make_compute(&mut stage, x);
    let use_t = stage.statement().definition(BuilderDialect::Use(t)).new();
    let then_ret = stage.statement().definition(BuilderDialect::Return).new();
    let then_block = stage
        .block()
        .stmt(compute)
        .stmt(use_t)
        .terminator(then_ret)
        .new();
    let use_x = stage.statement().definition(BuilderDialect::Use(x)).new();
    let else_ret = stage.statement().definition(BuilderDialect::Return).new();
    let else_block = stage.block().stmt(use_x).terminator(else_ret).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(then_block, else_block))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let entry = stage.block().stmt(split).stmt(owner).terminator(ret).new();
    let stage = stage.finalize().unwrap();
    IfLike {
        stage,
        entry,
        x,
        then_block,
        t,
        compute,
        use_t,
        owner,
    }
}

#[test]
fn clone_statement_copies_nested_bodies_with_fresh_ids() {
    let IfLike {
        mut stage,
        entry,
        x,
        then_block,
        t,
        compute,
        use_t,
        owner,
    } = if_like();
    let mut rw = Rewriter::new(&mut stage);
    let mut map = IRMap::default();
    let copy = rw.clone_statement(owner, &mut map).unwrap();

    // Detached until inserted, and nothing reachable changed.
    assert_eq!(*copy.parent(rw.stage()), None);
    assert!(rw.events().is_empty());
    rw.insert(InsertionPoint::After(owner), copy).unwrap();
    assert_eq!(order(rw.stage(), entry)[1..], [owner, copy]);
    assert_eq!(
        rw.events().to_vec(),
        vec![MutationEvent::InsertedStatement { stmt: copy }]
    );

    // The copy owns copies of the arms, which hold copies of the statements.
    assert_eq!(map.statement(owner), Some(copy));
    let then_copy = map.block(then_block).unwrap();
    assert_ne!(then_copy, then_block);
    assert_eq!(
        then_copy.expect_info(rw.stage()).parent(),
        Some(BlockParent::Statement(copy))
    );
    let (compute_copy, use_t_copy) = (
        map.statement(compute).unwrap(),
        map.statement(use_t).unwrap(),
    );
    assert_eq!(order(rw.stage(), then_copy), [compute_copy, use_t_copy]);
    // The copied result is new and read by the copied use; `%x` comes from
    // outside, so both computes read it.
    let t_copy = map.value(t).unwrap();
    assert_ne!(t_copy, t);
    assert_eq!(
        *definition(rw.stage(), use_t_copy),
        BuilderDialect::Use(t_copy)
    );
    assert_eq!(
        uses_set(rw.stage(), t_copy),
        HashSet::from([so(use_t_copy, 0)])
    );
    assert!(uses_set(rw.stage(), x).contains(&so(compute, 0)));
    assert!(uses_set(rw.stage(), x).contains(&so(compute_copy, 0)));
    verify_derived(rw.stage()).unwrap();
}

#[test]
fn a_clone_never_inserted_is_tombstoned_with_the_rewriter() {
    let IfLike {
        mut stage,
        x,
        compute,
        owner,
        ..
    } = if_like();
    let uses_before = uses_set(&stage, x);
    let (copy, compute_copy) = {
        let mut rw = Rewriter::new(&mut stage);
        let mut map = IRMap::default();
        let copy = rw.clone_statement(owner, &mut map).unwrap();
        (copy, map.statement(compute).unwrap())
    };
    assert!(copy.get_info(&stage).is_none());
    assert!(compute_copy.get_info(&stage).is_none());
    assert_eq!(uses_set(&stage, x), uses_before);
    verify_derived(&stage).unwrap();
}

#[test]
fn a_clone_cannot_be_inserted_inside_itself() {
    let IfLike {
        mut stage,
        use_t,
        owner,
        ..
    } = if_like();
    let mut rw = Rewriter::new(&mut stage);
    let mut map = IRMap::default();
    let copy = rw.clone_statement(owner, &mut map).unwrap();
    // The copy of `use_t` sits in a body of `copy`.
    let inside = map.statement(use_t).unwrap();
    let insertion_pt = InsertionPoint::Before(inside);
    assert_eq!(
        rw.insert(insertion_pt, copy),
        Err(RewriteError::CannotInsert {
            insertion_pt,
            reason: Box::new(RewriteError::InsertedIntoItself(copy)),
        })
    );
    assert!(rw.events().is_empty());
}

/// ```text
/// host: %z, _ = split; f = own_cfg body {
///   ^a: br ^c
///   ^b: u = use %v; ret
///   ^c: %v = compute %z; br ^b
/// }; ret
/// ```
/// `^b` reads a value from `^c`, listed after it, and `^a` branches forward.
struct ForwardCfg {
    stage: StageInfo<BuilderDialect>,
    host: Block,
    z: SSAValue,
    body: CFG,
    blocks: [Block; 3],
    u: Statement,
    v: SSAValue,
    f: Statement,
}

fn forward_cfg() -> ForwardCfg {
    let mut stage = new_stage();
    let (split, z, _) = make_split(&mut stage);
    let (compute, v) = make_compute(&mut stage, z);
    let u = stage.statement().definition(BuilderDialect::Use(v)).new();
    let b_ret = stage.statement().definition(BuilderDialect::Return).new();
    let b = stage.block().stmt(u).terminator(b_ret).new();
    let c_br = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(b)))
        .new();
    let c = stage.block().stmt(compute).terminator(c_br).new();
    let a_br = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(c)))
        .new();
    let a = stage.block().terminator(a_br).new();
    let body = stage.cfg().add_block(a).add_block(b).add_block(c).new();
    let f = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(body))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let host = stage.block().stmt(split).stmt(f).terminator(ret).new();
    let stage = stage.finalize().unwrap();
    ForwardCfg {
        stage,
        host,
        z,
        body,
        blocks: [a, b, c],
        u,
        v,
        f,
    }
}

#[test]
fn clone_cfg_maps_forward_branches_and_values_from_later_blocks() {
    let ForwardCfg {
        mut stage,
        host,
        z,
        body,
        blocks: [a, b, c],
        u,
        v,
        f,
    } = forward_cfg();
    let mut rw = Rewriter::new(&mut stage);
    let mut map = IRMap::default();
    let copy = rw.clone_cfg(body, &mut map).unwrap();
    assert!(rw.events().is_empty());

    let [a2, b2, c2] = [a, b, c].map(|block| map.block(block).unwrap());
    assert_eq!(copy.blocks(rw.stage()).collect::<Vec<_>>(), [a2, b2, c2]);
    // Branches point at the copies, and the predecessor mirror follows.
    assert_eq!(preds(rw.stage(), c2), HashSet::from([a2]));
    assert_eq!(preds(rw.stage(), b2), HashSet::from([c2]));
    // `^b`'s copy reads the copy of `%v`, defined in `^c`'s copy; `%z` is
    // shared.
    let (u2, v2) = (map.statement(u).unwrap(), map.value(v).unwrap());
    assert_eq!(*definition(rw.stage(), u2), BuilderDialect::Use(v2));
    assert_eq!(uses_set(rw.stage(), v2), HashSet::from([so(u2, 0)]));
    assert_eq!(uses_set(rw.stage(), z).len(), 2);

    // A new statement can own the copy, which is then no longer detached:
    // dropping the rewriter keeps it.
    let g = rw
        .create(InsertionPoint::After(f), BuilderDialect::OwnCFG(copy))
        .unwrap();
    drop(rw);
    assert_eq!(order(&stage, host)[1..], [f, g]);
    assert!(copy.get_info(&stage).is_some());
    verify_derived(&stage).unwrap();
}

#[test]
fn a_cloned_cfg_spliced_away_is_tombstoned_with_the_rewriter() {
    // callee { ^e(%a): use %a; ret }, inlined into caller { ^k: ret }.
    let mut stage = new_stage();
    let a = stage.block_argument().index(0);
    let use_a = stage.statement().definition(BuilderDialect::Use(a)).new();
    let e_ret = stage.statement().definition(BuilderDialect::Return).new();
    let e = stage
        .block()
        .argument(TestType::I32)
        .stmt(use_a)
        .terminator(e_ret)
        .new();
    let callee = stage.cfg().add_block(e).new();
    let k_ret = stage.statement().definition(BuilderDialect::Return).new();
    let k = stage
        .block()
        .argument(TestType::I32)
        .terminator(k_ret)
        .new();
    let caller = stage.cfg().add_block(k).new();
    let mut stage = stage.finalize().unwrap();
    let a: SSAValue = e.expect_info(&stage).arguments()[0].into();
    let w: SSAValue = k.expect_info(&stage).arguments()[0].into();

    let copy = {
        let mut rw = Rewriter::new(&mut stage);
        // The callee's entry argument reads the caller's `%w`, so the copied
        // entry block has no arguments.
        let mut map = IRMap::default();
        map.map_value(a, w);
        let copy = rw.clone_cfg(callee, &mut map).unwrap();
        let e2 = map.block(e).unwrap();
        assert!(e2.expect_info(rw.stage()).arguments().is_empty());
        let use_a2 = map.statement(use_a).unwrap();
        assert_eq!(*definition(rw.stage(), use_a2), BuilderDialect::Use(w));

        rw.splice_cfg_after(copy, k).unwrap();
        assert_eq!(caller.blocks(rw.stage()).collect::<Vec<_>>(), [k, e2]);
        copy
    };
    // Emptied by the splice and still detached: tombstoned.
    assert!(copy.get_info(&stage).is_none());
    verify_derived(&stage).unwrap();
}

#[test]
fn clones_reject_without_mutating() {
    let mut stage = new_stage();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let member = stage.statement().definition(BuilderDialect::Nop).new();
    let dg = stage.digraph().node(member).new();
    let graph_owner = stage
        .statement()
        .definition(BuilderDialect::OwnDiGraph(dg))
        .new();
    // A graph body nested one level down.
    let inner = stage.block().stmt(graph_owner).new();
    let empty = stage.block().new();
    let outer = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(inner, empty))
        .new();
    let _entry = stage.block().stmt(outer).terminator(ret).new();
    let mut stage = stage.finalize().unwrap();
    let sizes = |stage: &StageInfo<BuilderDialect>| {
        (
            stage.statement_arena().len(),
            stage.ssa_arena().len(),
            stage.block_arena().len(),
        )
    };
    let before = sizes(&stage);
    let dead_stmt = stage.statement_arena().next_id();
    let dead_cfg = CFG::from(Id::from(TestSSAValue(9_999)));

    let mut rw = Rewriter::new(&mut stage);
    let mut map = IRMap::default();
    assert_eq!(
        rw.clone_statement(dead_stmt, &mut map),
        Err(RewriteError::UnknownStatement(dead_stmt))
    );
    assert_eq!(
        rw.clone_statement(ret, &mut map),
        Err(RewriteError::UnexpectedTerminator)
    );
    assert_eq!(
        rw.clone_statement(outer, &mut map),
        Err(RewriteError::CopyOfGraphBody(graph_owner))
    );
    assert_eq!(
        rw.clone_cfg(dead_cfg, &mut map),
        Err(RewriteError::UnknownCfg(dead_cfg))
    );
    assert!(rw.events().is_empty());
    assert_eq!(sizes(rw.stage()), before);
}

#[test]
fn create_adopts_only_unowned_bodies() {
    let ForwardCfg {
        mut stage,
        body,
        blocks: [_, b, _],
        u,
        f,
        ..
    } = forward_cfg();
    let mut rw = Rewriter::new(&mut stage);
    // `body` belongs to `f`.
    assert_eq!(
        rw.create(InsertionPoint::After(f), BuilderDialect::OwnCFG(body)),
        Err(RewriteError::BodyOwnedElsewhere(Body::CFG(body)))
    );
    let mut map = IRMap::default();
    let copy = rw.clone_cfg(body, &mut map).unwrap();
    // A body cannot hold its own owner.
    let inside = map.statement(u).unwrap();
    assert_eq!(
        rw.create(InsertionPoint::Before(inside), BuilderDialect::OwnCFG(copy)),
        Err(RewriteError::BodyEnclosesOwner(Body::CFG(copy)))
    );
    // A block of a CFG is owned by the CFG.
    let b2 = map.block(b).unwrap();
    assert_eq!(
        rw.create(InsertionPoint::After(f), BuilderDialect::OwnBlocks(b2, b2)),
        Err(RewriteError::BodyOwnedElsewhere(Body::Block(b2)))
    );
    assert!(rw.events().is_empty());
}

#[test]
fn a_clone_rejects_a_dead_replacement_only_if_it_reads_it() {
    let IfLike {
        mut stage,
        x,
        owner,
        ..
    } = if_like();
    let dead = SSAValue::from(TestSSAValue(9_999));
    let ssas_before = stage.ssa_arena().len();
    let mut rw = Rewriter::new(&mut stage);
    // The copy reads `%x`, whose replacement is dead.
    let mut map = IRMap::default();
    map.map_value(x, dead);
    assert_eq!(
        rw.clone_statement(owner, &mut map),
        Err(RewriteError::UnknownValue(dead))
    );
    assert_eq!(rw.stage().ssa_arena().len(), ssas_before);
    // A dead replacement the copy never reads does not matter.
    let mut map = IRMap::default();
    map.map_value(dead, dead);
    rw.clone_statement(owner, &mut map).unwrap();
}
