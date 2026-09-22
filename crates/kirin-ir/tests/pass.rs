//! Integration tests for [`run_pass`], the rewrite-pass ownership boundary.
//!
//! The boundary has one success route and three failure routes, and the tests
//! are grouped that way: the pass returns a value, the pass returns an error,
//! the pass panics, or the pass finishes but leaves derived metadata stale.
//! Only the first hands back a [`StageInfo`]; the other three yield a
//! [`Quarantined`] the caller cannot rewrite or re-enter.
//!
//! Two properties are asserted repeatedly because they are the ones easiest to
//! regress:
//!
//! - **no rollback** — edits made before a failure are still in the stage and
//!   still named by the event log;
//! - **events outlive the closure** — [`run_pass`] builds the [`Rewriter`]
//!   before running the pass, so a panic caused by the pass does not take
//!   the edit history with it.

// Every `run_pass` result carries a stage on both sides; see the note on
// `run_pass` itself for why the `Err` variant is deliberately large.
#![allow(clippy::result_large_err)]

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

/// One block with two arguments, an `add` reading both, and an operand-less
/// `nop`. `nop` is the handle for provoking a rejected edit: it has no operand
/// slots, so any `replace_operand` on it fails without touching the stage.
struct Fixture {
    stage: StageInfo<BuilderDialect>,
    block: Block,
    add: Statement,
    nop: Statement,
}

impl Fixture {
    fn x(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(&self.stage).arguments[0])
    }

    fn y(&self) -> SSAValue {
        SSAValue::from(self.block.expect_info(&self.stage).arguments[1])
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

    Fixture {
        stage: stage.finalize().unwrap(),
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
    let f = fixture();
    let (x, y, add) = (f.x(), f.y(), f.add);

    let (stage, replaced) = run_pass(f.stage, |rewriter| rewriter.replace_operand(add, 1, x))
        .expect("a pass making only legal edits should succeed");

    // The pass's own return value comes back untouched.
    assert_eq!(replaced, y, "replace_operand returns the previous operand");
    // The edit landed.
    assert_eq!(
        add.expect_info(&stage).definition(),
        &BuilderDialect::Add(x, x)
    );
    // And the stage is ordinary IR again, mirrors included.
    assert_eq!(verify_derived(&stage), Ok(()));
}

#[test]
fn a_stage_returned_by_one_pass_can_be_handed_to_the_next() {
    let f = fixture();
    let (x, y, add) = (f.x(), f.y(), f.add);

    // Ownership goes out and comes back, so passes chain without any
    // "is this stage currently being rewritten" bookkeeping.
    let (stage, _) = run_pass(f.stage, |rewriter| rewriter.replace_operand(add, 1, x))
        .expect("first pass should succeed");
    let (stage, _) = run_pass(stage, |rewriter| rewriter.replace_operand(add, 0, y))
        .expect("second pass should succeed");

    assert_eq!(
        add.expect_info(&stage).definition(),
        &BuilderDialect::Add(y, x)
    );
    assert_eq!(verify_derived(&stage), Ok(()));
}

// ---------------------------------------------------------------------------
// The pass returns an error
// ---------------------------------------------------------------------------

#[test]
fn a_pass_that_returns_an_error_quarantines_the_stage() {
    let f = fixture();
    let (x, add, nop) = (f.x(), f.add, f.nop);

    let quarantined = run_pass(f.stage, |rewriter| -> Result<(), RewriteError> {
        rewriter.replace_operand(add, 1, x)?;
        // `Nop` declares no operands, so this is rejected.
        rewriter.replace_operand(nop, 0, x)?;
        Ok(())
    })
    .expect_err("an erroring pass must not hand back a usable stage");

    let QuarantineCause::Pass(error) = quarantined.cause() else {
        panic!("expected a pass-error cause, got {:?}", quarantined.cause());
    };
    assert_eq!(
        error.to_string(),
        RewriteError::OperandIndexOutOfRange {
            stmt: nop,
            index: 0
        }
        .to_string()
    );
}

#[test]
fn a_quarantined_stage_keeps_the_edits_made_before_the_failure() {
    let f = fixture();
    let (x, add, nop) = (f.x(), f.add, f.nop);

    let quarantined = run_pass(f.stage, |rewriter| -> Result<(), RewriteError> {
        rewriter.replace_operand(add, 1, x)?;
        rewriter.replace_operand(nop, 0, x)?;
        Ok(())
    })
    .expect_err("expected the second edit to fail");

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
    x.get_info_mut(&mut f.stage).unwrap().uses_mut().clear();

    let quarantined = run_pass(f.stage, |_| Ok::<(), RewriteError>(()))
        .expect_err("a stale mirror must not survive the boundary");

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
    x.get_info_mut(&mut f.stage).unwrap().uses_mut().clear();

    let quarantined = run_pass(f.stage, |rewriter| rewriter.replace_operand(nop, 0, x))
        .expect_err("the pass error alone should quarantine the stage");

    // Verification never ran, so the cause names what actually went wrong
    // rather than a stale mirror that was already there.
    assert!(matches!(quarantined.cause(), QuarantineCause::Pass(_)));
}

// ---------------------------------------------------------------------------
// The pass panics
// ---------------------------------------------------------------------------

#[test]
fn a_panicking_pass_is_quarantined_with_the_edits_it_already_made() {
    let f = fixture();
    let (x, add) = (f.x(), f.add);

    let quarantined = without_panic_output(|| {
        run_pass(f.stage, |rewriter| -> Result<(), RewriteError> {
            rewriter.replace_operand(add, 1, x)?;
            panic!("rule invariant violated")
        })
    })
    .expect_err("a panicking pass must not hand back a usable stage");

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
    let f = fixture();
    let add = f.add;

    let quarantined = without_panic_output(|| {
        run_pass(f.stage, |_| -> Result<(), RewriteError> {
            panic!("unexpected statement {add:?}")
        })
    })
    .expect_err("a panicking pass must not hand back a usable stage");

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
    let f = fixture();

    let quarantined = without_panic_output(|| {
        run_pass(f.stage, |_| -> Result<(), RewriteError> {
            std::panic::panic_any(7u32)
        })
    })
    .expect_err("a panicking pass must not hand back a usable stage");

    let QuarantineCause::Panic(message) = quarantined.cause() else {
        panic!("expected a panic cause, got {:?}", quarantined.cause());
    };
    assert_eq!(message, "panicked with a non-string payload");
}
