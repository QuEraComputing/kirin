//! `Rewriter::new_cfg` makes a CFG owned by nothing, to become a statement's
//! body. Inserting a statement that names it makes the statement its parent;
//! one still unowned when the `Rewriter` drops is tombstoned, and
//! `Rewriter::erase_body` discards one earlier.

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
