//! What the pass boundary does with metadata that was already stale.
//!
//! In-crate because both tests have to desync a mirror first, which a pass
//! cannot do through a [`Rewriter`](crate::Rewriter). The rest of the
//! boundary's behaviour is covered through the public API in `tests/pass.rs`.

use crate::arena::GetInfo;
use crate::testing::dialect::{TestLang, TestType, new_stage};
use crate::{
    Block, CompileStage, Mismatch, Pipeline, QuarantineCause, Quarantined, RewriteError, Rewriter,
    SSAValue, StageInfo, StagePassError, Statement, VerifyError,
};

/// Empty a value's use list while its operand slots still read it.
///
/// A pass cannot do this through the `Rewriter` — which is the point — so this
/// stands in for a mutation path that bypassed the boundary entirely.
fn clear_uses(stage: &mut StageInfo<TestLang>, value: SSAValue) {
    value
        .get_info_mut(stage)
        .expect("live SSA value")
        .uses
        .clear();
}

/// A one-stage pipeline holding one block with two arguments, an `add` reading
/// both, and an operand-less `nop` to aim a rejected edit at.
struct Fixture {
    pipeline: Pipeline<StageInfo<TestLang>>,
    id: CompileStage,
    block: Block,
    nop: Statement,
}

fn fixture() -> Fixture {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let y = stage.block_argument().index(1);
    let add = stage.statement().definition(TestLang::Add(x, y)).new();
    let nop = stage.statement().definition(TestLang::Nop).new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(add)
        .stmt(nop)
        .new();

    let mut pipeline: Pipeline<StageInfo<TestLang>> = Pipeline::new();
    let id = pipeline.add_stage_raw(stage.finalize().unwrap());

    Fixture {
        pipeline,
        id,
        block,
        nop,
    }
}

impl Fixture {
    fn stage(&self) -> &StageInfo<TestLang> {
        self.pipeline.stage(self.id).expect("stage is still usable")
    }

    fn stage_mut(&mut self) -> &mut StageInfo<TestLang> {
        self.pipeline
            .stage_mut(self.id)
            .expect("stage is still usable")
    }

    fn quarantined(&self) -> &Quarantined<StageInfo<TestLang>> {
        self.pipeline
            .quarantined(self.id)
            .expect("the position was poisoned")
    }

    fn x(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(self.stage()).arguments[0])
    }

    fn run_pass<F, T, E>(&mut self, pass: F) -> Result<T, StagePassError>
    where
        E: std::error::Error + Send + Sync + 'static,
        F: FnOnce(&mut Rewriter<TestLang>) -> Result<T, E>,
    {
        self.pipeline.run_pass::<TestLang, _, _, _>(self.id, pass)
    }
}

#[test]
fn a_stale_mirror_is_caught_at_the_pass_boundary() {
    let mut f = fixture();
    let x = f.x();

    // Desync before the pass runs. A pass cannot do this through the
    // `Rewriter` — which is the point — so this stands in for a mutation path
    // that bypassed the boundary entirely.
    clear_uses(f.stage_mut(), x);

    f.run_pass(|_| Ok::<(), RewriteError>(()))
        .expect_err("a stale mirror must not survive the boundary");

    let quarantined = f.quarantined();
    let QuarantineCause::Verify(VerifyError::Mismatch(mismatches)) = quarantined.cause() else {
        panic!("expected a mirror mismatch, got {:?}", quarantined.cause());
    };
    assert!(matches!(
        mismatches.as_slice(),
        [Mismatch::Uses { value, .. }] if *value == x
    ));
}

#[test]
fn a_pass_error_is_reported_even_when_the_mirrors_are_also_stale() {
    let mut f = fixture();
    let (x, nop) = (f.x(), f.nop);
    clear_uses(f.stage_mut(), x);

    f.run_pass(|rewriter| rewriter.replace_operand(nop, 0, x))
        .expect_err("the pass error alone should quarantine the stage");

    // Verification never ran, so the cause names what actually went wrong
    // rather than a stale mirror that was already there.
    assert!(matches!(f.quarantined().cause(), QuarantineCause::Pass(_)));
}
