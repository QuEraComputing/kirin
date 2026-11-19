//! `Rewriter::new_cfg`, `new_digraph` and `new_ungraph` make bodies owned by
//! nothing, to become a statement's body. Inserting a statement that names
//! one makes the statement its parent; one still unowned when the `Rewriter`
//! drops is tombstoned, and `Rewriter::erase_body` discards one earlier.

mod common;

use common::{BuilderDialect, TestType, new_stage};
use kirin_ir::*;

/// `anchor = nop` in `outer`, whose argument `x` the new bodies read.
struct Fixture {
    stage: StageInfo<BuilderDialect>,
    anchor: Statement,
    x: SSAValue,
}

fn fixture() -> Fixture {
    let mut stage = new_stage();
    let anchor = stage.statement().definition(BuilderDialect::Nop).new();
    let outer = stage.block().argument(TestType::I32).stmt(anchor).new();
    let stage = stage.finalize().unwrap();
    let x = outer.expect_info(&stage).arguments()[0].into();
    Fixture { stage, anchor, x }
}

/// Fill `cfg` with `^a(i32): use(x); br ^b` and `^b: return`, returning
/// `(^a, ^b, use(x))`.
fn fill_cfg(
    rewriter: &mut Rewriter<'_, BuilderDialect>,
    cfg: CFG,
    x: SSAValue,
) -> (Block, Block, Statement) {
    let a = rewriter.append_block(cfg, [TestType::I32]).unwrap();
    let b = rewriter.append_block(cfg, []).unwrap();
    let br = rewriter
        .insert_terminator(a, BuilderDialect::Branch(Successor::from_block(b)))
        .unwrap();
    rewriter
        .insert_terminator(b, BuilderDialect::Return)
        .unwrap();
    let use_x = rewriter.insert_before(br, BuilderDialect::Use(x)).unwrap();
    (a, b, use_x)
}

fn uses_of(stage: &StageInfo<BuilderDialect>, value: SSAValue) -> usize {
    value.expect_info(stage).uses().len()
}

#[test]
fn new_cfg_is_empty_and_records_no_event() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let cfg = rewriter.new_cfg();

    assert_eq!(cfg.blocks(rewriter.stage()).count(), 0);
    assert_eq!(
        rewriter.events(),
        [],
        "a CFG owned by nothing is not part of the IR yet"
    );
}

#[test]
fn inserted_statement_adopts_a_new_cfg() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let cfg = rewriter.new_cfg();
    let (a, b, use_x) = fill_cfg(&mut rewriter, cfg, f.x);

    let owner = rewriter
        .insert_after(f.anchor, BuilderDialect::OwnCFG(cfg))
        .unwrap();
    assert_eq!(
        rewriter.events().last(),
        Some(&MutationEvent::InsertedStatement { stmt: owner })
    );
    assert_eq!(
        rewriter.insert_after(owner, BuilderDialect::OwnCFG(cfg)),
        Err(RewriteError::BodyOwnedElsewhere(Body::CFG(cfg))),
        "the CFG has an owner now"
    );

    drop(rewriter);
    for block in [a, b] {
        assert!(block.get_info(&f.stage).is_some(), "an adopted CFG is kept");
    }
    assert!(use_x.get_info(&f.stage).is_some());
    assert_eq!(cfg.blocks(&f.stage).collect::<Vec<_>>(), [a, b]);
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn unowned_new_cfg_is_tombstoned_on_drop() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let cfg = rewriter.new_cfg();
    let (a, b, use_x) = fill_cfg(&mut rewriter, cfg, f.x);
    let arg: SSAValue = a.expect_info(rewriter.stage()).arguments()[0].into();
    assert_eq!(uses_of(rewriter.stage(), f.x), 1);
    drop(rewriter);

    assert!(cfg.get_info(&f.stage).is_none());
    for block in [a, b] {
        assert!(block.get_info(&f.stage).is_none());
    }
    assert!(use_x.get_info(&f.stage).is_none());
    assert!(arg.get_info(&f.stage).is_none());
    assert_eq!(uses_of(&f.stage, f.x), 0, "the use of x is dropped");
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erase_body_discards_an_unowned_new_cfg() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let cfg = rewriter.new_cfg();
    let (a, b, _) = fill_cfg(&mut rewriter, cfg, f.x);
    let events_before = rewriter.events().len();

    rewriter.erase_body(cfg).unwrap();
    assert_eq!(
        rewriter.events().len(),
        events_before,
        "erasing a body owned by nothing records no event"
    );
    assert!(cfg.get_info(rewriter.stage()).is_none());
    for block in [a, b] {
        assert!(block.get_info(rewriter.stage()).is_none());
    }
    assert_eq!(uses_of(rewriter.stage(), f.x), 0);
    drop(rewriter);
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erase_body_rejects_a_new_cfg_still_referenced_from_outside() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);

    // An argument of its block read outside it.
    let cfg = rewriter.new_cfg();
    let (a, _, _) = fill_cfg(&mut rewriter, cfg, f.x);
    let arg: SSAValue = a.expect_info(rewriter.stage()).arguments()[0].into();
    let reader = rewriter
        .insert_after(f.anchor, BuilderDialect::Use(arg))
        .unwrap();
    assert_eq!(
        rewriter.erase_body(cfg),
        Err(RewriteError::BlockArgumentInUse { block: a, index: 0 })
    );
    rewriter.erase_statement(reader).unwrap();

    // A block outside it branching into it.
    let source = rewriter.new_block([]);
    rewriter
        .insert_terminator(source, BuilderDialect::Branch(Successor::from_block(a)))
        .unwrap();
    assert_eq!(
        rewriter.erase_body(cfg),
        Err(RewriteError::BlockHasPredecessors {
            block: a,
            pred: source
        })
    );
    assert!(
        cfg.get_info(rewriter.stage()).is_some(),
        "a rejected erase changes nothing"
    );
}

#[test]
fn a_statement_in_a_new_cfg_cannot_adopt_that_cfg() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let cfg = rewriter.new_cfg();
    let (_, _, use_x) = fill_cfg(&mut rewriter, cfg, f.x);

    assert_eq!(
        rewriter.insert_after(use_x, BuilderDialect::OwnCFG(cfg)),
        Err(RewriteError::BodyEnclosesOwner(Body::CFG(cfg)))
    );
}

fn port_types(stage: &StageInfo<BuilderDialect>, ports: &[Port]) -> Vec<TestType> {
    ports
        .iter()
        .map(|&port| SSAValue::from(port).expect_info(stage).ty().clone())
        .collect()
}

#[test]
fn new_graphs_have_their_ports_and_record_no_event() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let dg = rewriter.new_digraph([TestType::I32, TestType::I64], [TestType::Any]);
    let ug = rewriter.new_ungraph([TestType::Qubit], []);

    let stage = rewriter.stage();
    let info = dg.expect_info(stage);
    assert_eq!(
        port_types(stage, info.edge_ports()),
        [TestType::I32, TestType::I64]
    );
    assert_eq!(port_types(stage, info.capture_ports()), [TestType::Any]);
    assert_eq!(info.nodes().count(), 0);
    assert_eq!(info.yields(), []);
    assert_eq!(info.parent(), None);
    let info = ug.expect_info(stage);
    assert_eq!(port_types(stage, info.edge_ports()), [TestType::Qubit]);
    assert_eq!(info.capture_ports(), []);
    assert_eq!(info.parent(), None);
    assert_eq!(
        rewriter.events(),
        [],
        "a graph owned by nothing is not part of the IR yet"
    );
}

/// Fill `dg` with `use(x)` and `ug` with a wire `w` and `gate(w, port0)`,
/// returning `(use(x), w's edge statement, gate)`.
fn fill_graphs(
    rewriter: &mut Rewriter<'_, BuilderDialect>,
    dg: DiGraph,
    ug: UnGraph,
    x: SSAValue,
) -> (Statement, Statement, Statement) {
    let use_x = rewriter.insert_node(dg, BuilderDialect::Use(x)).unwrap();
    let w = rewriter.new_result(TestType::Qubit);
    let wire = rewriter
        .insert_ungraph_edge(ug, BuilderDialect::Wire(w))
        .unwrap();
    let port: SSAValue = ug.expect_info(rewriter.stage()).edge_ports()[0].into();
    let gate = rewriter
        .insert_ungraph_node(ug, BuilderDialect::Gate(w.into(), port))
        .unwrap();
    (use_x, wire, gate)
}

#[test]
fn inserted_statement_adopts_new_graphs() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let dg = rewriter.new_digraph([], []);
    let ug = rewriter.new_ungraph([TestType::Qubit], []);
    let members = fill_graphs(&mut rewriter, dg, ug, f.x);

    let owner = rewriter
        .insert_after(f.anchor, BuilderDialect::OwnGraphs(dg, ug))
        .unwrap();
    assert_eq!(
        rewriter.events().last(),
        Some(&MutationEvent::InsertedStatement { stmt: owner })
    );
    assert_eq!(dg.expect_info(rewriter.stage()).parent(), Some(owner));
    assert_eq!(ug.expect_info(rewriter.stage()).parent(), Some(owner));

    drop(rewriter);
    for stmt in [members.0, members.1, members.2] {
        assert!(stmt.get_info(&f.stage).is_some(), "adopted graphs are kept");
    }
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn unowned_new_graphs_are_tombstoned_on_drop() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let dg = rewriter.new_digraph([TestType::I32], []);
    let ug = rewriter.new_ungraph([TestType::Qubit], []);
    let members = fill_graphs(&mut rewriter, dg, ug, f.x);
    let ports: Vec<SSAValue> = [
        dg.expect_info(rewriter.stage()).ports(),
        ug.expect_info(rewriter.stage()).ports(),
    ]
    .concat()
    .into_iter()
    .map(SSAValue::from)
    .collect();
    assert_eq!(uses_of(rewriter.stage(), f.x), 1);
    drop(rewriter);

    assert!(dg.get_info(&f.stage).is_none());
    assert!(ug.get_info(&f.stage).is_none());
    for stmt in [members.0, members.1, members.2] {
        assert!(stmt.get_info(&f.stage).is_none());
    }
    for port in ports {
        assert!(port.get_info(&f.stage).is_none());
    }
    assert_eq!(uses_of(&f.stage, f.x), 0, "the use of x is dropped");
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erase_body_discards_unowned_new_graphs() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let dg = rewriter.new_digraph([], []);
    let ug = rewriter.new_ungraph([TestType::Qubit], []);
    fill_graphs(&mut rewriter, dg, ug, f.x);
    let events_before = rewriter.events().len();

    rewriter.erase_body(dg).unwrap();
    rewriter.erase_body(ug).unwrap();
    assert_eq!(
        rewriter.events().len(),
        events_before,
        "erasing a body owned by nothing records no event"
    );
    assert!(dg.get_info(rewriter.stage()).is_none());
    assert!(ug.get_info(rewriter.stage()).is_none());
    assert_eq!(uses_of(rewriter.stage(), f.x), 0);
    drop(rewriter);
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erase_body_rejects_a_new_graph_whose_values_are_used_outside() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let dg = rewriter.new_digraph([TestType::I32], []);
    let port = dg.expect_info(rewriter.stage()).edge_ports()[0];

    // A port read outside the graph.
    let reader = rewriter
        .insert_after(f.anchor, BuilderDialect::Use(port.into()))
        .unwrap();
    assert_eq!(rewriter.erase_body(dg), Err(RewriteError::PortInUse(port)));
    rewriter.erase_statement(reader).unwrap();

    // A node's result read outside the graph.
    let r = rewriter.new_result(TestType::I32);
    let node = rewriter
        .insert_node(dg, BuilderDialect::Compute(port.into(), r))
        .unwrap();
    rewriter
        .insert_after(f.anchor, BuilderDialect::Use(r.into()))
        .unwrap();
    assert_eq!(
        rewriter.erase_body(dg),
        Err(RewriteError::StatementResultsInUse(node))
    );
    assert!(
        dg.get_info(rewriter.stage()).is_some(),
        "a rejected erase changes nothing"
    );
}

#[test]
fn a_new_digraph_gets_its_yields_from_set_yields() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let dg = rewriter.new_digraph([TestType::I32], []);
    let port: SSAValue = dg.expect_info(rewriter.stage()).edge_ports()[0].into();
    let r = rewriter.new_result(TestType::I32);
    rewriter
        .insert_node(dg, BuilderDialect::Compute(port, r))
        .unwrap();
    rewriter.set_yields(dg, [r]).unwrap();
    rewriter
        .insert_after(f.anchor, BuilderDialect::OwnDiGraph(dg))
        .unwrap();
    drop(rewriter);

    assert_eq!(dg.expect_info(&f.stage).yields(), [SSAValue::from(r)]);
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erase_body_counts_yields_of_the_erased_graph_as_inside_it() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let dg = rewriter.new_digraph([TestType::I32], []);
    let port: SSAValue = dg.expect_info(rewriter.stage()).edge_ports()[0].into();
    let r = rewriter.new_result(TestType::I32);
    let node = rewriter
        .insert_node(dg, BuilderDialect::Compute(port, r))
        .unwrap();
    // The node's result is used only by the graph's own yield.
    rewriter.set_yields(dg, [r]).unwrap();

    rewriter.erase_body(dg).unwrap();
    assert!(dg.get_info(rewriter.stage()).is_none());
    assert!(node.get_info(rewriter.stage()).is_none());
    drop(rewriter);
    assert_eq!(verify_derived(&f.stage), Ok(()));
}
