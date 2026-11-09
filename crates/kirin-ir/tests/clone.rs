//! Cloning statements and CFGs with the Rewriter.

mod common;

use std::collections::{HashMap, HashSet};

use common::{BuilderDialect, TestType, make_compute, make_split, make_wire, new_stage};
use kirin_ir::*;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

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
        // Mapping the callee's entry argument has no effect: it is copied.
        let mut map = IRMap::default();
        map.map_value(a, w);
        let copy = rw.clone_cfg(callee, &mut map).unwrap();
        let e2 = map.block(e).unwrap();
        let a2: SSAValue = e2.expect_info(rw.stage()).arguments()[0].into();
        assert_eq!(map.value(a), Some(a2));
        let use_a2 = map.statement(use_a).unwrap();
        assert_eq!(*definition(rw.stage(), use_a2), BuilderDialect::Use(a2));

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
    let _entry = stage.block().terminator(ret).new();
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
        rw.clone_cfg(dead_cfg, &mut map),
        Err(RewriteError::UnknownCfg(dead_cfg))
    );
    assert!(rw.events().is_empty());
    assert_eq!(sizes(rw.stage()), before);
}

#[test]
fn a_clone_copies_every_block_argument_and_can_reuse_its_map() {
    // ^entry(%x): jump ^next(%x); ^next(%a): use %a; ret
    let mut stage = new_stage();
    let (split, z, _) = make_split(&mut stage);
    let a = stage.block_argument().index(0);
    let use_a = stage.statement().definition(BuilderDialect::Use(a)).new();
    let next_ret = stage.statement().definition(BuilderDialect::Return).new();
    let next = stage
        .block()
        .argument(TestType::I32)
        .stmt(use_a)
        .terminator(next_ret)
        .new();
    let x = stage.block_argument().index(0);
    let jump = stage
        .statement()
        .definition(BuilderDialect::Jump(Successor::from_block(next), vec![x]))
        .new();
    let entry = stage.block().argument(TestType::I32).terminator(jump).new();
    let body = stage.cfg().add_block(entry).add_block(next).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(body))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let host = stage.block().stmt(split).stmt(owner).terminator(ret).new();
    let mut stage = stage.finalize().unwrap();
    let a: SSAValue = next.expect_info(&stage).arguments()[0].into();

    let mut rw = Rewriter::new(&mut stage);
    // Mapping `%a` has no effect: `^next`'s copy keeps its argument, which
    // the copied jump still passes.
    let mut map = IRMap::default();
    map.map_value(a, z);
    let mut copy = || {
        let copy = rw.clone_cfg(body, &mut map).unwrap();
        let [entry2, next2] = [entry, next].map(|block| map.block(block).unwrap());
        let x2: SSAValue = entry2.expect_info(rw.stage()).arguments()[0].into();
        let a2: SSAValue = next2.expect_info(rw.stage()).arguments()[0].into();
        let jump2 = map.statement(jump).unwrap();
        assert_eq!(
            *definition(rw.stage(), jump2),
            BuilderDialect::Jump(Successor::from_block(next2), vec![x2])
        );
        let use_a2 = map.statement(use_a).unwrap();
        assert_eq!(*definition(rw.stage(), use_a2), BuilderDialect::Use(a2));
        rw.create(InsertionPoint::After(owner), BuilderDialect::OwnCFG(copy))
            .unwrap()
    };
    // A second copy with the same map is independent of the first.
    let first = copy();
    let second = copy();
    drop(rw);
    assert_eq!(order(&stage, host)[1..], [owner, second, first]);
    verify_derived(&stage).unwrap();
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

#[test]
fn a_clone_reuses_a_map_whose_earlier_copy_is_gone() {
    let IfLike {
        mut stage,
        x,
        t,
        use_t,
        owner,
        ..
    } = if_like();
    let mut map = IRMap::default();
    // The first copy is tombstoned with its rewriter, so `map` takes `%t` to
    // a dead value.
    let first_t = {
        let mut rw = Rewriter::new(&mut stage);
        rw.clone_statement(owner, &mut map).unwrap();
        map.value(t).unwrap()
    };
    assert!(first_t.get_info(&stage).is_none());

    // The copy defines `%t` again, so the dead entry is never read.
    let mut rw = Rewriter::new(&mut stage);
    let copy = rw.clone_statement(owner, &mut map).unwrap();
    let t2 = map.value(t).unwrap();
    assert_ne!(t2, first_t);
    let use_t2 = map.statement(use_t).unwrap();
    assert_eq!(*definition(rw.stage(), use_t2), BuilderDialect::Use(t2));
    assert_eq!(map.value(x), None);
    rw.insert(InsertionPoint::After(owner), copy).unwrap();
    drop(rw);
    verify_derived(&stage).unwrap();
}

#[test]
fn a_clone_of_a_clone_reads_its_own_values() {
    let IfLike {
        mut stage,
        entry,
        x,
        t,
        compute,
        use_t,
        owner,
        ..
    } = if_like();
    let mut rw = Rewriter::new(&mut stage);
    let mut map = IRMap::default();
    let first = rw.clone_statement(owner, &mut map).unwrap();
    rw.insert(InsertionPoint::After(owner), first).unwrap();
    let (t1, compute1, use_t1) = (
        map.value(t).unwrap(),
        map.statement(compute).unwrap(),
        map.statement(use_t).unwrap(),
    );

    let second = rw.clone_statement(first, &mut map).unwrap();
    rw.insert(InsertionPoint::After(first), second).unwrap();
    // The first copy's entries are left as they were; the second copy's are
    // keyed by the first copy's values.
    assert_eq!(map.value(t), Some(t1));
    let (t2, compute2, use_t2) = (
        map.value(t1).unwrap(),
        map.statement(compute1).unwrap(),
        map.statement(use_t1).unwrap(),
    );
    assert_ne!(t2, t1);
    assert_eq!(*definition(rw.stage(), use_t2), BuilderDialect::Use(t2));
    assert_eq!(
        *definition(rw.stage(), compute2),
        BuilderDialect::Compute(x, t2.into())
    );
    drop(rw);
    assert_eq!(order(&stage, entry)[1..], [owner, first, second]);
    verify_derived(&stage).unwrap();
}

/// A `DiGraph`'s petgraph edges, as `(producer, consumer, value)`.
fn digraph_edges(
    stage: &StageInfo<BuilderDialect>,
    graph: DiGraph,
) -> HashSet<(Statement, Statement, SSAValue)> {
    let petgraph = graph.expect_info(stage).graph();
    petgraph
        .edge_references()
        .map(|edge| {
            (
                petgraph[edge.source()],
                petgraph[edge.target()],
                *edge.weight(),
            )
        })
        .collect()
}

/// An `UnGraph`'s petgraph edges: each wire with its two endpoints.
fn ungraph_edges(
    stage: &StageInfo<BuilderDialect>,
    graph: UnGraph,
) -> HashMap<SSAValue, HashSet<Statement>> {
    let petgraph = graph.expect_info(stage).graph();
    petgraph
        .edge_references()
        .map(|edge| {
            let ends = HashSet::from([petgraph[edge.source()], petgraph[edge.target()]]);
            (*edge.weight(), ends)
        })
        .collect()
}

#[test]
fn clone_statement_copies_a_digraph_with_ports_edges_and_yields() {
    // entry: %x, _ = split
    //        owner = own_digraph dg(%port) { p: %rp = compute %x; q: add %rp, %port; yield %rp }
    let mut stage = new_stage();
    let (split, x, _) = make_split(&mut stage);
    let (p, rp) = make_compute(&mut stage, x);
    let dg = stage
        .digraph()
        .port(TestType::I32)
        .node(p)
        .yield_value(rp)
        .new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnDiGraph(dg))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let entry = stage.block().stmt(split).stmt(owner).terminator(ret).new();
    let mut stage = stage.finalize().unwrap();
    let port = SSAValue::from(dg.expect_info(&stage).ports()[0]);

    let mut rw = Rewriter::new(&mut stage);
    let q = rw
        .create(InsertionPoint::DiGraph(dg), BuilderDialect::Add(rp, port))
        .unwrap();
    let mut map = IRMap::default();
    let copy = rw.clone_statement(owner, &mut map).unwrap();
    rw.insert(InsertionPoint::After(owner), copy).unwrap();
    assert_eq!(order(rw.stage(), entry), [split, owner, copy]);

    let BuilderDialect::OwnDiGraph(dg2) = *definition(rw.stage(), copy) else {
        panic!("the copy owns a DiGraph");
    };
    assert_ne!(dg2, dg);
    let info = dg2.expect_info(rw.stage());
    // Fresh ports, members, edges and yields; `%x` from outside is shared.
    let port2 = SSAValue::from(info.ports()[0]);
    assert_eq!(map.value(port), Some(port2));
    assert_eq!(
        *port2.expect_info(rw.stage()).kind(),
        SSAKind::Port(PortParent::DiGraph(dg2), 0)
    );
    let (p2, q2, rp2) = (
        map.statement(p).unwrap(),
        map.statement(q).unwrap(),
        map.value(rp).unwrap(),
    );
    assert_eq!(
        info.members().map(|(stmt, _)| stmt).collect::<Vec<_>>(),
        [p2, q2]
    );
    assert_eq!(*definition(rw.stage(), q2), BuilderDialect::Add(rp2, port2));
    assert_eq!(info.yields(), [rp2]);
    assert_eq!(
        digraph_edges(rw.stage(), dg2),
        HashSet::from([(p2, q2, rp2)])
    );
    assert_eq!(
        uses_set(rw.stage(), rp2),
        HashSet::from([
            so(q2, 0),
            Use::DiGraphYield {
                graph: dg2,
                index: 0
            }
        ])
    );
    assert!(uses_set(rw.stage(), x).contains(&so(p2, 0)));
    drop(rw);
    verify_derived(&stage).unwrap();
}

#[test]
fn clone_statement_copies_an_ungraph_with_its_wires() {
    // entry: owner = own_ungraph ug(%b) { w = wire; a = gate w, %b; c = use w }; ret
    let mut stage = new_stage();
    let (w_stmt, w) = make_wire(&mut stage);
    let ug = stage.ungraph().port(TestType::Qubit).edge(w_stmt).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnUnGraph(ug))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let _entry = stage.block().stmt(owner).terminator(ret).new();
    let mut stage = stage.finalize().unwrap();
    let boundary = SSAValue::from(ug.expect_info(&stage).edge_ports()[0]);

    let mut rw = Rewriter::new(&mut stage);
    let a = rw
        .create(
            InsertionPoint::UnGraph(ug),
            BuilderDialect::Gate(w, boundary),
        )
        .unwrap();
    let c = rw
        .create(InsertionPoint::UnGraph(ug), BuilderDialect::Use(w))
        .unwrap();
    let mut map = IRMap::default();
    let copy = rw.clone_statement(owner, &mut map).unwrap();
    rw.insert(InsertionPoint::After(owner), copy).unwrap();

    let BuilderDialect::OwnUnGraph(ug2) = *definition(rw.stage(), copy) else {
        panic!("the copy owns an UnGraph");
    };
    let (w2_stmt, w2, a2, c2) = (
        map.statement(w_stmt).unwrap(),
        map.value(w).unwrap(),
        map.statement(a).unwrap(),
        map.statement(c).unwrap(),
    );
    let info = ug2.expect_info(rw.stage());
    assert_eq!(
        info.members()
            .map(|(stmt, member)| (stmt, member.role()))
            .collect::<Vec<_>>(),
        [
            (w2_stmt, MemberRole::Edge),
            (a2, MemberRole::Node),
            (c2, MemberRole::Node)
        ]
    );
    // The copied wire connects the copied nodes; the copied boundary port has
    // one reader, so no edge.
    let boundary2 = map.value(boundary).unwrap();
    assert_eq!(
        *definition(rw.stage(), a2),
        BuilderDialect::Gate(w2, boundary2)
    );
    assert_eq!(
        ungraph_edges(rw.stage(), ug2),
        HashMap::from([(w2, HashSet::from([a2, c2]))])
    );
    drop(rw);
    verify_derived(&stage).unwrap();
}

#[test]
fn clone_statement_copies_a_graph_nested_in_a_block() {
    // entry: outer = own_blocks { inner: owner = own_digraph { nop } } { }; ret
    let mut stage = new_stage();
    let member = stage.statement().definition(BuilderDialect::Nop).new();
    let dg = stage.digraph().node(member).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnDiGraph(dg))
        .new();
    let inner = stage.block().stmt(owner).new();
    let empty = stage.block().new();
    let outer = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(inner, empty))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let _entry = stage.block().stmt(outer).terminator(ret).new();
    let mut stage = stage.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    let mut map = IRMap::default();
    let copy = rw.clone_statement(outer, &mut map).unwrap();
    rw.insert(InsertionPoint::After(outer), copy).unwrap();
    let owner2 = map.statement(owner).unwrap();
    let BuilderDialect::OwnDiGraph(dg2) = *definition(rw.stage(), owner2) else {
        panic!("the copied owner owns a DiGraph");
    };
    assert_ne!(dg2, dg);
    assert_eq!(
        dg2.expect_info(rw.stage())
            .members()
            .map(|(stmt, _)| stmt)
            .collect::<Vec<_>>(),
        [map.statement(member).unwrap()]
    );
    drop(rw);
    verify_derived(&stage).unwrap();
}
