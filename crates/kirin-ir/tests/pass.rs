//! Integration tests for [`Pipeline::run_pass`], the rewrite-pass ownership
//! boundary.
//!
//! The boundary has one success route and three failure routes, and the tests
//! are grouped that way: the pass returns a value, the pass returns an error,
//! the pass panics, or the pass finishes but leaves derived metadata stale.
//! Only the first leaves the stage in its slot; the other three poison the
//! position, so the stage can be inspected through
//! [`Pipeline::quarantined`] but never rewritten or re-entered.
//!
//! Two properties are asserted repeatedly because they are the ones easiest to
//! regress:
//!
//! - **no rollback** — edits made before a failure are still in the stage and
//!   still named by the event log;
//! - **events outlive the closure** — the boundary builds the [`Rewriter`]
//!   before running the pass, so a panic caused by the pass does not take
//!   the edit history with it.
//!
//! [`StagePassError`] distinguishes the *outcomes*; the
//! [`QuarantineCause`] behind a [`StagePassError::PassFailed`] distinguishes
//! the three ways a pass can fail, and is what most of these tests read.

mod common;

use common::{BuilderDialect, TestType, new_stage};
use kirin_ir::*;

/// Run `f` with the panic hook silenced.
///
/// [`catch_unwind`](std::panic::catch_unwind) stops the unwind but leaves the
/// default hook installed, so a deliberate panic still prints to stderr and
/// makes a passing test read like a failing one. The hook is restored
/// afterwards — if `f` itself panicked the restore would be skipped, but `f` is
/// always a `run_pass` call, which catches.
fn without_panic_output<T>(f: impl FnOnce() -> T) -> T {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = f();
    std::panic::set_hook(previous);
    result
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

/// A one-stage pipeline whose stage holds one block with two arguments, an
/// `add` reading both, and an operand-less `nop`. `nop` is the handle for
/// provoking a rejected edit: it has no operand slots, so any `replace_operand`
/// on it fails without touching the stage.
///
/// The stage is a bare [`StageInfo`], which is its own stage container —
/// `StageInfo<L>` implements both [`StageMeta`] and [`HasStageInfo<L>`] — so a
/// single-dialect pipeline needs no enum.
struct Fixture {
    pipeline: Pipeline<StageInfo<BuilderDialect>>,
    id: CompileStage,
    block: Block,
    add: Statement,
    nop: Statement,
}

impl Fixture {
    /// The stage still in its slot. Panics once a failed pass has poisoned it.
    fn stage(&self) -> &StageInfo<BuilderDialect> {
        self.pipeline.stage(self.id).expect("stage is still usable")
    }

    fn stage_mut(&mut self) -> &mut StageInfo<BuilderDialect> {
        self.pipeline
            .stage_mut(self.id)
            .expect("stage is still usable")
    }

    /// The artifact a failed pass left at this position.
    fn quarantined(&self) -> &Quarantined<StageInfo<BuilderDialect>> {
        self.pipeline
            .quarantined(self.id)
            .expect("the position was poisoned")
    }

    fn x(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(self.stage()).arguments[0])
    }

    fn y(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(self.stage()).arguments[1])
    }

    fn run_pass<F, T, E>(&mut self, pass: F) -> Result<T, StagePassError>
    where
        E: std::error::Error + Send + Sync + 'static,
        F: FnOnce(&mut Rewriter<BuilderDialect>) -> Result<T, E>,
    {
        self.pipeline
            .run_pass::<BuilderDialect, _, _, _>(self.id, pass)
    }
}

fn fixture() -> Fixture {
    let mut stage = new_stage();

    let x = stage.block_argument().index(0);
    let y = stage.block_argument().index(1);
    let add = stage
        .statement()
        .definition(BuilderDialect::Add(x, y))
        .new();
    let nop = stage.statement().definition(BuilderDialect::Nop).new();
    let block = stage
        .block()
        .argument(TestType::I32)
        .argument(TestType::I32)
        .stmt(add)
        .stmt(nop)
        .new();

    let mut pipeline: Pipeline<StageInfo<BuilderDialect>> = Pipeline::new();
    let id = pipeline.add_stage_raw(stage.finalize().unwrap());

    Fixture {
        pipeline,
        id,
        block,
        add,
        nop,
    }
}

// ---------------------------------------------------------------------------
// Success
// ---------------------------------------------------------------------------

#[test]
fn a_successful_pass_returns_the_stage_and_the_pass_value() {
    let mut f = fixture();
    let (x, y, add) = (f.x(), f.y(), f.add);

    let replaced = f
        .run_pass(|rewriter| rewriter.replace_operand(add, 1, x))
        .expect("a pass making only legal edits should succeed");

    // The pass's own return value comes back untouched.
    assert_eq!(replaced, y, "replace_operand returns the previous operand");
    // The edit landed.
    assert_eq!(
        add.expect_info(f.stage()).definition(),
        &BuilderDialect::Add(x, x)
    );
    // And the stage is ordinary IR again, back in its slot, mirrors included.
    assert_eq!(verify_derived(f.stage()), Ok(()));
    assert!(f.pipeline.quarantined(f.id).is_none());
}

#[test]
fn a_stage_returned_by_one_pass_can_be_handed_to_the_next() {
    let mut f = fixture();
    let (x, y, add) = (f.x(), f.y(), f.add);

    // The stage leaves its slot and comes back, so passes chain without any
    // "is this stage currently being rewritten" bookkeeping.
    f.run_pass(|rewriter| rewriter.replace_operand(add, 1, x))
        .expect("first pass should succeed");
    f.run_pass(|rewriter| rewriter.replace_operand(add, 0, y))
        .expect("second pass should succeed");

    assert_eq!(
        add.expect_info(f.stage()).definition(),
        &BuilderDialect::Add(y, x)
    );
    assert_eq!(verify_derived(f.stage()), Ok(()));
}

// ---------------------------------------------------------------------------
// The pass returns an error
// ---------------------------------------------------------------------------

#[test]
fn a_pass_that_returns_an_error_quarantines_the_stage() {
    let mut f = fixture();
    let (x, add, nop) = (f.x(), f.add, f.nop);

    let error = f
        .run_pass(|rewriter| -> Result<(), RewriteError> {
            rewriter.replace_operand(add, 1, x)?;
            // `Nop` declares no operands, so this is rejected.
            rewriter.replace_operand(nop, 0, x)?;
            Ok(())
        })
        .expect_err("an erroring pass must not hand back a usable stage");

    assert_eq!(error, StagePassError::PassFailed);
    assert!(
        f.pipeline.stage(f.id).is_none(),
        "the position is poisoned, so its stage is not lent out again"
    );

    let quarantined = f.quarantined();
    let QuarantineCause::Pass(cause) = quarantined.cause() else {
        panic!("expected a pass-error cause, got {:?}", quarantined.cause());
    };
    assert_eq!(
        cause.to_string(),
        RewriteError::OperandIndexOutOfRange {
            stmt: nop,
            index: 0
        }
        .to_string()
    );
}

#[test]
fn a_quarantined_stage_keeps_the_edits_made_before_the_failure() {
    let mut f = fixture();
    let (x, add, nop) = (f.x(), f.add, f.nop);

    f.run_pass(|rewriter| -> Result<(), RewriteError> {
        rewriter.replace_operand(add, 1, x)?;
        rewriter.replace_operand(nop, 0, x)?;
        Ok(())
    })
    .expect_err("expected the second edit to fail");

    let quarantined = f.quarantined();
    // There is no rollback: the first edit happened and is reported as such.
    assert_eq!(
        quarantined.events().to_vec(),
        vec![MutationEvent::ChangedOperands { stmt: add }],
    );
    // The diagnostic dump is the other half — the events name ids, the report
    // is what makes those ids mean something.
    assert!(quarantined.report().contains("ChangedOperands"));
}

// ---------------------------------------------------------------------------
// The pass leaves derived metadata stale
// ---------------------------------------------------------------------------

#[test]
fn a_stale_mirror_is_caught_at_the_pass_boundary() {
    let mut f = fixture();
    let x = f.x();

    // Desync before the pass runs. A pass cannot do this through the
    // `Rewriter` — which is the point — so this stands in for a mutation path
    // that bypassed the boundary entirely.
    x.get_info_mut(f.stage_mut()).unwrap().uses_mut().clear();

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
    x.get_info_mut(f.stage_mut()).unwrap().uses_mut().clear();

    f.run_pass(|rewriter| rewriter.replace_operand(nop, 0, x))
        .expect_err("the pass error alone should quarantine the stage");

    // Verification never ran, so the cause names what actually went wrong
    // rather than a stale mirror that was already there.
    assert!(matches!(f.quarantined().cause(), QuarantineCause::Pass(_)));
}

// ---------------------------------------------------------------------------
// The pass panics
// ---------------------------------------------------------------------------

#[test]
fn a_panicking_pass_is_quarantined_with_the_edits_it_already_made() {
    let mut f = fixture();
    let (x, add) = (f.x(), f.add);

    without_panic_output(|| {
        f.run_pass(|rewriter| -> Result<(), RewriteError> {
            rewriter.replace_operand(add, 1, x)?;
            panic!("rule invariant violated")
        })
    })
    .expect_err("a panicking pass must not hand back a usable stage");

    let quarantined = f.quarantined();
    let QuarantineCause::Panic(message) = quarantined.cause() else {
        panic!("expected a panic cause, got {:?}", quarantined.cause());
    };
    assert_eq!(message, "rule invariant violated");

    // The edit made before the panic is still in the log
    assert_eq!(
        quarantined.events().to_vec(),
        vec![MutationEvent::ChangedOperands { stmt: add }],
    );
}

#[test]
fn a_formatted_panic_message_survives_the_boundary() {
    let mut f = fixture();
    let add = f.add;

    without_panic_output(|| {
        f.run_pass(|_| -> Result<(), RewriteError> { panic!("unexpected statement {add:?}") })
    })
    .expect_err("a panicking pass must not hand back a usable stage");

    let quarantined = f.quarantined();
    let QuarantineCause::Panic(message) = quarantined.cause() else {
        panic!("expected a panic cause, got {:?}", quarantined.cause());
    };
    // `panic!` boxes a bare literal as `&'static str` but anything formatted as
    // a `String`, so this covers the downcast arm the test above does not.
    assert!(
        message.contains("unexpected statement"),
        "formatted panic message was lost: {message:?}"
    );
}

#[test]
fn a_non_string_panic_payload_still_produces_a_cause() {
    let mut f = fixture();

    without_panic_output(|| {
        f.run_pass(|_| -> Result<(), RewriteError> { std::panic::panic_any(7u32) })
    })
    .expect_err("a panicking pass must not hand back a usable stage");

    let quarantined = f.quarantined();
    let QuarantineCause::Panic(message) = quarantined.cause() else {
        panic!("expected a panic cause, got {:?}", quarantined.cause());
    };
    assert_eq!(message, "panicked with a non-string payload");
}
