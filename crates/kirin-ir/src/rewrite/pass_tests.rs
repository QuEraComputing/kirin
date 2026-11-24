//! What the pass boundary does with metadata that was already stale, and with
//! detached code a pass leaves behind.
//!
//! In-crate because the stale-metadata tests have to desync a mirror first,
//! which a pass cannot do through a [`Rewriter`](crate::Rewriter), and the
//! detached-code tests need [`Rewriter::build_statement`], which no public API
//! exposes yet. The rest of the boundary's behaviour is covered through the
//! public API in `tests/pass.rs`.

use crate::arena::{GetInfo, GetInfoMut};
use crate::testing::dialect::{TestLang, TestType, new_stage};
use crate::{
    Block, CompileStage, Finding, InsertionPoint, Mismatch, Program, QuarantineCause, Quarantined,
    RewriteError, Rewriter, SSAValue, StageInfo, StagePassError, Statement, Use, VerifyError,
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

/// A one-stage program holding one block with two arguments, an `add` reading
/// both, and an operand-less `nop` to aim a rejected edit at.
struct Fixture {
    program: Program<StageInfo<TestLang>>,
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

    let mut program: Program<StageInfo<TestLang>> = Program::new();
    let id = program.add_stage_raw(stage.finalize().unwrap());

    Fixture {
        program,
        id,
        block,
        nop,
    }
}

impl Fixture {
    fn stage(&self) -> &StageInfo<TestLang> {
        self.program.stage(self.id).expect("stage is still usable")
    }

    fn stage_mut(&mut self) -> &mut StageInfo<TestLang> {
        self.program
            .stage_mut(self.id)
            .expect("stage is still usable")
    }

    fn quarantined(&self) -> &Quarantined<StageInfo<TestLang>> {
        self.program
            .quarantined(self.id)
            .expect("the position was poisoned")
    }

    fn x(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(self.stage()).arguments()[0])
    }

    fn run_pass<F, T, E>(&mut self, pass: F) -> Result<T, StagePassError>
    where
        E: std::error::Error + Send + Sync + 'static,
        F: FnOnce(&mut Rewriter<TestLang>) -> Result<T, E>,
    {
        self.program.run_pass::<TestLang, _, _, _>(self.id, pass)
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

#[test]
fn a_panicking_pass_leaves_its_detached_code_in_place() {
    let mut f = fixture();
    let mut detached = None;

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let (_, result) =
        crate::rewrite::pass::drive_pass(f.stage_mut(), |rw| -> Result<(), RewriteError> {
            let block = rw.new_block([]);
            let ret = rw.create(InsertionPoint::Terminator(block), TestLang::Return)?;
            let nop = rw.create(InsertionPoint::Before(ret), TestLang::Nop)?;
            // Leave the block half-built, as a panic part-way through an edit
            // could: its chain still names a statement whose slot is gone.
            let _ = rw.stage.statement_arena_mut().delete(nop);
            detached = Some(block);
            panic!("rule gave up")
        });
    std::panic::set_hook(previous_hook);

    assert!(matches!(result, Err(QuarantineCause::Panic(_))));
    let block = detached.expect("the pass made a block before panicking");
    assert!(
        block.get_info(f.stage()).is_some(),
        "the drop left the detached block for inspection instead of tombstoning it"
    );
}

#[test]
fn a_panicking_pass_leaves_its_detached_statements_in_place() {
    let mut f = fixture();
    let x = f.x();
    let mut built = None;

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let (_, result) =
        crate::rewrite::pass::drive_pass(f.stage_mut(), |rw| -> Result<(), RewriteError> {
            let reader = rw.build_statement(TestLang::Use(x))?;
            let value = rw.new_result(TestType::I32);
            let def = rw.build_statement(TestLang::Def(value))?;
            built = Some((reader, def, SSAValue::from(value)));
            panic!("rule gave up")
        });
    std::panic::set_hook(previous_hook);

    assert!(matches!(result, Err(QuarantineCause::Panic(_))));
    let (reader, def, value) = built.expect("the pass built statements before panicking");
    let stage = f.stage();
    assert!(reader.get_info(stage).is_some());
    assert!(def.get_info(stage).is_some());
    assert!(value.get_info(stage).is_some());
    assert!(
        x.expect_info(stage)
            .uses()
            .contains(&Use::StatementOperand {
                stmt: reader,
                index: 0
            }),
        "the detached reader kept its use of x"
    );
}

#[test]
fn reading_a_result_of_a_discarded_statement_quarantines_the_stage() {
    let mut f = fixture();
    let nop = f.nop;
    let mut built = None;

    // `def` is never inserted, so the drop tombstones it with `value`, which
    // the placed `reader` still reads.
    f.run_pass(|rw| -> Result<(), RewriteError> {
        let value = rw.new_result(TestType::I32);
        rw.build_statement(TestLang::Def(value))?;
        let value = SSAValue::from(value);
        let reader = rw.create(InsertionPoint::Before(nop), TestLang::Use(value))?;
        built = Some((reader, value));
        Ok(())
    })
    .expect_err("a dangling operand must not survive the boundary");

    let (reader, value) = built.expect("the pass ran to completion");
    let QuarantineCause::Verify(VerifyError::Derive(error)) = f.quarantined().cause() else {
        panic!(
            "expected a derive finding, got {:?}",
            f.quarantined().cause()
        );
    };
    assert_eq!(
        error.findings(),
        [Finding::DanglingOperand {
            stmt: reader,
            index: 0,
            value
        }]
    );
}
