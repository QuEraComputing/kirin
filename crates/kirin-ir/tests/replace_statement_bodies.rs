//! `Rewriter::replace_statement` keeps body parents in step with the
//! definition that names them.

mod common;

use common::{BuilderDialect, EventCursor, new_stage};
use kirin_ir::*;

/// The statements and blocks of a small stage, by name.
struct Fixture {
    stage: StageInfo<BuilderDialect>,
    /// `owner = own_blocks { then_nop } { else_nop }`, in a block `outer`.
    owner: Statement,
    then_block: Block,
    then_nop: Statement,
    else_block: Block,
    else_nop: Statement,
    /// `other = own_blocks { .. } { .. }`, also in `outer`.
    other: Statement,
    other_block: Block,
    /// A block the builder left owned by nothing, holding a `nop`. A
    /// `Rewriter` did not create it, so it cannot be adopted.
    spare: Block,
}

fn fixture() -> Fixture {
    let mut stage = new_stage();
    let then_nop = stage.statement().definition(BuilderDialect::Nop).new();
    let then_block = stage.block().stmt(then_nop).new();
    let else_nop = stage.statement().definition(BuilderDialect::Nop).new();
    let else_block = stage.block().stmt(else_nop).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(then_block, else_block))
        .new();

    let other_block = stage.block().new();
    let other_second = stage.block().new();
    let other = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(other_block, other_second))
        .new();

    let spare_nop = stage.statement().definition(BuilderDialect::Nop).new();
    let spare = stage.block().stmt(spare_nop).new();
    // The block `owner` and `other` sit in.
    let _outer = stage.block().stmt(owner).stmt(other).new();

    Fixture {
        stage: stage.finalize().unwrap(),
        owner,
        then_block,
        then_nop,
        else_block,
        else_nop,
        other,
        other_block,
        spare,
    }
}

fn parent(stage: &StageInfo<BuilderDialect>, block: Block) -> Option<BlockParent> {
    block.expect_info(stage).parent()
}

#[test]
fn swapping_the_bodies_keeps_them() {
    let mut f = fixture();
    let events = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter
            .replace_statement(
                f.owner,
                BuilderDialect::OwnBlocks(f.else_block, f.then_block),
            )
            .unwrap();
        rewriter.events().to_vec()
    };
    assert_eq!(
        events,
        [MutationEvent::ReplacedStatement { stmt: f.owner }],
        "the same bodies, so no body change is recorded"
    );

    for block in [f.then_block, f.else_block] {
        assert_eq!(
            parent(&f.stage, block),
            Some(BlockParent::Statement(f.owner))
        );
    }
    assert!(f.then_nop.get_info(&f.stage).is_some());
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

#[test]
fn a_dropped_body_is_erased_and_a_gained_one_adopted() {
    let mut f = fixture();
    let (gained, gained_ret, events) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let gained = rewriter.new_block([]);
        let gained_ret = rewriter
            .insert_terminator(gained, BuilderDialect::Return)
            .unwrap();
        let mut cursor = EventCursor::default();
        cursor.advance(&rewriter);
        rewriter
            .replace_statement(f.owner, BuilderDialect::OwnBlocks(f.then_block, gained))
            .unwrap();
        (gained, gained_ret, cursor.read_and_advance(&rewriter))
    };
    assert_eq!(
        events,
        [
            MutationEvent::ReplacedStatement { stmt: f.owner },
            MutationEvent::ChangedBodyNesting { owner: f.owner },
        ]
    );

    // The else-block went with what was in it.
    assert!(f.else_block.get_info(&f.stage).is_none());
    assert!(f.else_nop.get_info(&f.stage).is_none());
    // The gained block now belongs to `owner`.
    assert_eq!(
        parent(&f.stage, gained),
        Some(BlockParent::Statement(f.owner))
    );
    assert!(gained_ret.get_info(&f.stage).is_some());
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

/// Replace `owner`'s definition with `definition` and check it is rejected
/// with `error`, leaving the stage as it was.
fn assert_rejected(f: &mut Fixture, definition: BuilderDialect, error: RewriteError) {
    let before = f.owner.expect_info(&f.stage).definition().clone();
    let (result, events) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let result = rewriter.replace_statement(f.owner, definition);
        (result, rewriter.events().to_vec())
    };
    assert_eq!(result, Err(error));
    assert!(events.is_empty());
    assert_eq!(*f.owner.expect_info(&f.stage).definition(), before);
    assert_eq!(
        parent(&f.stage, f.else_block),
        Some(BlockParent::Statement(f.owner))
    );
}

#[test]
fn a_body_owned_by_another_statement_is_rejected() {
    let mut f = fixture();
    let (then_block, other_block) = (f.then_block, f.other_block);
    assert_rejected(
        &mut f,
        BuilderDialect::OwnBlocks(then_block, other_block),
        RewriteError::BodyOwnedElsewhere(Body::Block(other_block)),
    );
    assert_eq!(
        parent(&f.stage, other_block),
        Some(BlockParent::Statement(f.other))
    );
}

#[test]
fn a_body_named_twice_is_rejected() {
    let mut f = fixture();
    let then_block = f.then_block;
    assert_rejected(
        &mut f,
        BuilderDialect::OwnBlocks(then_block, then_block),
        RewriteError::BodyListedTwice(Body::Block(then_block)),
    );
}

#[test]
fn a_body_enclosing_the_statement_is_rejected() {
    let mut f = fixture();
    let mut rewriter = Rewriter::new(&mut f.stage);
    // `nop` sits in `host`, so `host` cannot become one of its bodies.
    let host = rewriter.new_block([]);
    let ret = rewriter
        .insert_terminator(host, BuilderDialect::Return)
        .unwrap();
    let nop = rewriter.insert_before(ret, BuilderDialect::Nop).unwrap();
    let other = rewriter.new_block([]);
    let events_before = rewriter.events().len();
    assert_eq!(
        rewriter.replace_statement(nop, BuilderDialect::OwnBlocks(host, other)),
        Err(RewriteError::BodyEnclosesOwner(Body::Block(host)))
    );
    assert_eq!(rewriter.events().len(), events_before);
}

#[test]
fn a_body_the_builder_left_unowned_is_rejected() {
    let mut f = fixture();
    let (then_block, spare) = (f.then_block, f.spare);
    assert_rejected(
        &mut f,
        BuilderDialect::OwnBlocks(then_block, spare),
        RewriteError::BodyNotDetached(Body::Block(spare)),
    );
}

#[test]
fn an_erased_body_is_rejected() {
    let mut f = fixture();
    let (then_block, other_block) = (f.then_block, f.other_block);
    Rewriter::new(&mut f.stage)
        .erase_statement(f.other)
        .unwrap();
    assert_rejected(
        &mut f,
        BuilderDialect::OwnBlocks(then_block, other_block),
        RewriteError::UnknownBody(Body::Block(other_block)),
    );
}
