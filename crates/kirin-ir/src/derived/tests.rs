//! Mismatch detection.
//!
//! These live in-crate because they have to forge a state no correct
//! [`Rewriter`](crate::Rewriter) can produce where the authoritative IR is
//! intact, but the mirror over it is lying.

use std::collections::HashSet;

use crate::arena::GetInfo;
use crate::testing::dialect::{TestLang, TestType, new_stage};
use crate::{
    Block, LinkedList, Mismatch, SSAValue, StageInfo, Statement, Use, VerifyError, verify_derived,
};

/// Empty a value's use list while its operand slots still read it.
///
/// No `Rewriter` edit can produce this so the only way to check
/// that the mismatch is reported is to forge it.
fn clear_uses(stage: &mut StageInfo<TestLang>, value: SSAValue) {
    value
        .get_info_mut(stage)
        .expect("live SSA value")
        .uses
        .clear();
}

/// Empty a block's statement summary while its `prev`/`next` links still
/// describe the same chain. Forged for the same reason as [`clear_uses`].
fn clear_block_statements(stage: &mut StageInfo<TestLang>, block: Block) {
    block.get_info_mut(stage).expect("live block").statements = LinkedList::new();
}

fn operand(stmt: Statement, index: usize) -> Use {
    Use::StatementOperand { stmt, index }
}

/// One block whose two arguments are read by an `add` and a `use`, so `x` has
/// two recorded uses and `y` has one.
struct OperandStage {
    stage: StageInfo<TestLang>,
    block: Block,
    add: Statement,
    consumer: Statement,
}

fn operand_stage() -> OperandStage {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let y = stage.block_argument().index(1);
    let add = stage.statement().definition(TestLang::Add(x, y)).new();
    let consumer = stage.statement().definition(TestLang::Use(x)).new();
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
}

/// One block holding a three-statement chain plus a terminator.
struct BodyStage {
    stage: StageInfo<TestLang>,
    block: Block,
}

fn body_stage() -> BodyStage {
    let mut stage = new_stage();

    let first = stage.statement().definition(TestLang::Nop).new();
    let middle = stage.statement().definition(TestLang::Nop).new();
    let last = stage.statement().definition(TestLang::Nop).new();
    let terminator = stage.statement().definition(TestLang::Return).new();
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
    }
}

#[test]
fn a_hand_corrupted_use_index_is_reported_as_a_mismatch() {
    let mut f = operand_stage();
    let x = f.x();

    // Corrupt the mirror only — both operand slots still read `x`.
    clear_uses(&mut f.stage, x);

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

#[test]
fn a_hand_corrupted_block_body_is_reported_as_a_mismatch() {
    let mut f = body_stage();

    // Corrupt the mirror only — the `prev`/`next` links still describe the
    // same three-statement chain.
    clear_block_statements(&mut f.stage, f.block);

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
