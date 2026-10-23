//! `Rewriter::replace_statement` keeps body parents in step with the
//! definition that names them.

mod common;

use common::{BuilderDialect, new_stage};
use kirin_ir::*;

/// The statements and blocks of a small stage, by name.
struct Fixture {
    stage: StageInfo<BuilderDialect>,
    /// `owner = own_blocks { then_nop } { else_nop }`, in `outer`.
    owner: Statement,
    then_block: Block,
    then_nop: Statement,
    else_block: Block,
    else_nop: Statement,
    /// `other = own_blocks { .. } { .. }`, also in `outer`.
    other: Statement,
    other_block: Block,
    /// A block nothing owns, holding `spare_nop`.
    spare: Block,
    spare_nop: Statement,
    /// The unowned block `owner` and `other` sit in.
    outer: Block,
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
    let outer = stage.block().stmt(owner).stmt(other).new();

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
        spare_nop,
        outer,
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
        rewriter.drain_events()
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
    let events = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        rewriter
            .replace_statement(f.owner, BuilderDialect::OwnBlocks(f.then_block, f.spare))
            .unwrap();
        rewriter.drain_events()
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
    // The spare block now belongs to `owner`.
    assert_eq!(
        parent(&f.stage, f.spare),
        Some(BlockParent::Statement(f.owner))
    );
    assert!(f.spare_nop.get_info(&f.stage).is_some());
    assert_eq!(verify_derived(&f.stage), Ok(()));
}

/// Replace `owner`'s definition with `definition` and check it is rejected
/// with `error`, leaving the stage as it was.
fn assert_rejected(f: &mut Fixture, definition: BuilderDialect, error: RewriteError) {
    let before = f.owner.expect_info(&f.stage).definition().clone();
    let (result, events) = {
        let mut rewriter = Rewriter::new(&mut f.stage);
        let result = rewriter.replace_statement(f.owner, definition);
        (result, rewriter.drain_events())
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
    let (then_block, outer) = (f.then_block, f.outer);
    assert_rejected(
        &mut f,
        BuilderDialect::OwnBlocks(then_block, outer),
        RewriteError::BodyEnclosesOwner(Body::Block(outer)),
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
