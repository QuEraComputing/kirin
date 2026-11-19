//! `Rewriter::new_block` makes a block owned by nothing, to become a
//! statement's body; one still unowned when the `Rewriter` drops is
//! tombstoned. Inserting a statement that names such blocks makes it their
//! parent.

mod common;

use common::{BuilderDialect, EventCursor, TestType, new_stage};
use kirin_ir::*;

/// `owner = own_blocks { } { }` in `outer`, whose argument `x` the new blocks
/// read.
struct Fixture {
    stage: StageInfo<BuilderDialect>,
    owner: Statement,
    old_then: Block,
    old_else: Block,
    x: SSAValue,
}

fn fixture() -> Fixture {
    let mut stage = new_stage();
    let old_then = stage.block().new();
    let old_else = stage.block().new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(old_then, old_else))
        .new();
    let outer = stage.block().argument(TestType::I32).stmt(owner).new();
    let stage = stage.finalize().unwrap();
    let x = outer.expect_info(&stage).arguments()[0].into();
    Fixture {
        stage,
        owner,
        old_then,
        old_else,
        x,
    }
}

/// A stage holding only an empty `DiGraph` owned by nothing.
fn unowned_graph() -> (StageInfo<BuilderDialect>, DiGraph) {
    let mut stage = new_stage();
    let graph = stage.digraph().new();
    (stage.finalize().unwrap(), graph)
}

/// Fill `block` with `use(x)` then `return`.
fn fill(rewriter: &mut Rewriter<'_, BuilderDialect>, block: Block, x: SSAValue) -> Statement {
    let ret = rewriter
        .insert_terminator(block, BuilderDialect::Return)
        .unwrap();
    rewriter.insert_before(ret, BuilderDialect::Use(x)).unwrap()
}

#[test]
fn new_block_has_its_arguments_and_no_parent() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let block = rewriter.new_block([TestType::I32, TestType::I64]);

    let info = block.expect_info(rewriter.stage());
    assert_eq!(info.parent(), None);
    let types: Vec<TestType> = info
        .arguments()
        .iter()
        .map(|&arg| {
            SSAValue::from(arg)
                .expect_info(rewriter.stage())
                .ty()
                .clone()
        })
        .collect();
    assert_eq!(types, [TestType::I32, TestType::I64]);
    assert_eq!(
        rewriter.events(),
        [],
        "a block owned by nothing is not part of the IR yet"
    );
}

#[test]
fn adopted_new_blocks_become_the_owners_bodies() {
    let mut f = fixture();
    let (then_block, then_use, else_block, else_use) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let then_block = rewriter.new_block([]);
        let then_use = fill(&mut rewriter, then_block, f.x);
        let else_block = rewriter.new_block([]);
        let else_use = fill(&mut rewriter, else_block, f.x);
        rewriter
            .replace_statement(f.owner, BuilderDialect::OwnBlocks(then_block, else_block))
            .unwrap();
        (then_block, then_use, else_block, else_use)
    };

    for (block, stmt) in [(then_block, then_use), (else_block, else_use)] {
        assert_eq!(
            block.expect_info(&f.stage).parent(),
            Some(BlockParent::Statement(f.owner))
        );
        assert!(
            stmt.get_info(&f.stage).is_some(),
            "the drop keeps owned blocks"
        );
    }
    for block in [f.old_then, f.old_else] {
        assert!(
            block.get_info(&f.stage).is_none(),
            "the replaced bodies are erased"
        );
    }
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn unowned_new_block_is_tombstoned_on_drop() {
    let mut f = fixture();
    let (block, arg, used, events) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let block = rewriter.new_block([TestType::I32]);
        let arg: SSAValue = block.expect_info(rewriter.stage()).arguments()[0].into();
        let used = fill(&mut rewriter, block, f.x);
        (block, arg, used, rewriter.events().to_vec())
    };

    assert!(block.get_info(&f.stage).is_none());
    assert!(arg.get_info(&f.stage).is_none());
    assert!(used.get_info(&f.stage).is_none());
    assert!(
        f.x.expect_info(&f.stage).uses().is_empty(),
        "the tombstoned statement's use of `x` is dropped"
    );
    // Statements were inserted into it, so events were recorded; nothing
    // records the block's removal, since it never joined the IR.
    assert!(events.iter().all(|e| !matches!(
        e,
        MutationEvent::ChangedBlockList { .. } | MutationEvent::ChangedBodyNesting { .. }
    )));
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

fn parent(stage: &StageInfo<BuilderDialect>, block: Block) -> Option<BlockParent> {
    block.expect_info(stage).parent()
}

#[test]
fn inserted_statement_adopts_new_blocks() {
    let mut f = fixture();
    let (then_block, else_block, new_owner, events) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let mut cursor = EventCursor::default();
        let then_block = rewriter.new_block([]);
        fill(&mut rewriter, then_block, f.x);
        let else_block = rewriter.new_block([]);
        fill(&mut rewriter, else_block, f.x);
        cursor.advance(&rewriter);
        let new_owner = rewriter
            .insert_after(f.owner, BuilderDialect::OwnBlocks(then_block, else_block))
            .unwrap();
        (
            then_block,
            else_block,
            new_owner,
            cursor.read_and_advance(&rewriter),
        )
    };

    assert_eq!(
        events,
        [MutationEvent::InsertedStatement { stmt: new_owner }],
        "the insertion covers the bodies the statement gains"
    );
    for block in [then_block, else_block] {
        assert_eq!(
            parent(&f.stage, block),
            Some(BlockParent::Statement(new_owner))
        );
        assert_eq!(block.statements(&f.stage).count(), 1, "the drop keeps it");
    }
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn inserted_graph_member_adopts_new_blocks() {
    let (mut stage, graph) = unowned_graph();
    let (then_block, else_block, node) = {
        let mut rewriter = Rewriter::new(&mut stage);
        let then_block = rewriter.new_block([]);
        let else_block = rewriter.new_block([]);
        let node = rewriter
            .insert_node(graph, BuilderDialect::OwnBlocks(then_block, else_block))
            .unwrap();
        (then_block, else_block, node)
    };

    for block in [then_block, else_block] {
        assert_eq!(parent(&stage, block), Some(BlockParent::Statement(node)));
    }
}

#[test]
fn insert_rejects_a_body_owned_elsewhere() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let fresh = rewriter.new_block([]);
    assert_eq!(
        rewriter.insert_after(f.owner, BuilderDialect::OwnBlocks(fresh, f.old_then)),
        Err(RewriteError::BodyOwnedElsewhere(Body::Block(f.old_then)))
    );
    assert_eq!(rewriter.events(), [], "nothing was inserted");
    assert_eq!(parent(rewriter.stage(), fresh), None, "nothing was adopted");
}

#[test]
fn insert_rejects_a_body_named_twice() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    let fresh = rewriter.new_block([]);
    assert_eq!(
        rewriter.insert_after(f.owner, BuilderDialect::OwnBlocks(fresh, fresh)),
        Err(RewriteError::BodyListedTwice(Body::Block(fresh)))
    );
    assert_eq!(rewriter.events(), []);
}

#[test]
fn insert_rejects_a_body_holding_the_new_statement() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);

    // Into the block itself.
    let block = rewriter.new_block([]);
    let other = rewriter.new_block([]);
    let ret = rewriter
        .insert_terminator(block, BuilderDialect::Return)
        .unwrap();
    assert_eq!(
        rewriter.insert_before(ret, BuilderDialect::OwnBlocks(block, other)),
        Err(RewriteError::BodyEnclosesOwner(Body::Block(block)))
    );

    // Two levels down: `block` holds `holder`, which owns `inner`.
    let inner = rewriter.new_block([]);
    let inner_ret = rewriter
        .insert_terminator(inner, BuilderDialect::Return)
        .unwrap();
    let spare = rewriter.new_block([]);
    rewriter
        .insert_before(ret, BuilderDialect::OwnBlocks(inner, spare))
        .unwrap();
    assert_eq!(
        rewriter.insert_before(inner_ret, BuilderDialect::OwnBlocks(block, other)),
        Err(RewriteError::BodyEnclosesOwner(Body::Block(block)))
    );
    for unowned in [block, other] {
        assert_eq!(parent(rewriter.stage(), unowned), None);
    }
}

#[test]
fn insert_rejects_a_graph_member_owning_its_graph() {
    let (mut stage, graph) = unowned_graph();
    let mut rewriter = Rewriter::new(&mut stage);
    assert_eq!(
        rewriter.insert_node(graph, BuilderDialect::OwnDiGraph(graph)),
        Err(RewriteError::BodyEnclosesOwner(Body::DiGraph(graph)))
    );
    assert_eq!(rewriter.events(), []);
}

#[test]
fn erase_body_discards_an_unowned_new_block() {
    let mut f = fixture();
    let (block, used, events) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let mut cursor = EventCursor::default();
        let block = rewriter.new_block([TestType::I32]);
        let used = fill(&mut rewriter, block, f.x);
        cursor.advance(&rewriter);
        rewriter.erase_body(block).unwrap();
        (block, used, cursor.read_and_advance(&rewriter))
    };

    assert_eq!(events, [], "the block was never part of the IR");
    assert!(block.get_info(&f.stage).is_none());
    assert!(used.get_info(&f.stage).is_none());
    assert!(
        f.x.expect_info(&f.stage).uses().is_empty(),
        "the erased statement's use of `x` is dropped at once"
    );
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn erase_body_rejects_an_unowned_new_block_still_in_use() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);

    // Its argument read outside it.
    let block = rewriter.new_block([TestType::I32]);
    let arg: SSAValue = block.expect_info(rewriter.stage()).arguments()[0].into();
    rewriter
        .insert_after(f.owner, BuilderDialect::Use(arg))
        .unwrap();
    assert_eq!(
        rewriter.erase_body(block),
        Err(RewriteError::BlockArgumentInUse { block, index: 0 })
    );

    // Another block branching to it.
    let target = rewriter.new_block([]);
    let source = rewriter.new_block([]);
    rewriter
        .insert_terminator(
            source,
            BuilderDialect::Branch(Successor::from_block(target)),
        )
        .unwrap();
    assert_eq!(
        rewriter.erase_body(target),
        Err(RewriteError::BlockHasPredecessors {
            block: target,
            pred: source
        })
    );

    for kept in [block, target] {
        assert!(
            kept.get_info(rewriter.stage()).is_some(),
            "a rejected erase changes nothing"
        );
    }
}

#[test]
fn erase_body_rejects_bodies_it_cannot_erase_on_their_own() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    assert_eq!(
        rewriter.erase_body(f.old_then),
        Err(RewriteError::BodyOwnedElsewhere(Body::Block(f.old_then))),
        "a statement's body goes with the statement"
    );
    let block = rewriter.new_block([]);
    rewriter.erase_body(block).unwrap();
    assert_eq!(
        rewriter.erase_body(block),
        Err(RewriteError::UnknownBody(Body::Block(block)))
    );
    assert_eq!(rewriter.events(), []);
    drop(rewriter);
    assert!(f.old_then.get_info(&f.stage).is_some());

    let (mut stage, graph) = unowned_graph();
    let mut rewriter = Rewriter::new(&mut stage);
    assert_eq!(
        rewriter.erase_body(graph),
        Err(RewriteError::BodyNotDetached(Body::DiGraph(graph))),
        "a body the builder left unowned is not this rewriter's to erase"
    );
    drop(rewriter);
    assert!(graph.get_info(&stage).is_some());
}
