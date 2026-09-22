use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{Dialect, QuarantineCause, Quarantined, Rewriter, StageInfo, verify_derived};

/// [`run_pass`] runs a pass on a [`StageInfo`] and returns either the mutated
/// stage alongside the pass' result, or a [`Quarantined`] stage if the pass failed.
///
/// A Pass is a function that takes a mutable reference to a [`Rewriter`] and
/// returns a `Result`. The pass' `Result` type is left to be determined.
///
/// Note, that the argument is a [`Rewriter`] and not a [`StageInfo`](crate::StageInfo).
/// This is because the pass is expected to mutate the stage, and the
/// `Rewriter` already wraps a mutable reference to the `StageInfo`.
///
/// # Failure routes
///
/// Three distinct failures all quarantine the stage, and each records a
/// different [`QuarantineCause`]:
///
/// - the pass returns `Err` — [`QuarantineCause::Pass`]
/// - the pass panics — [`QuarantineCause::Panic`], caught here rather than in
///   the pass so the stage survives the unwind and can still be inspected;
/// - the pass succeeds but produces malformed IR/metadata — [`QuarantineCause::Verify`].
///
/// There is no rollback. Edits made before the failure stay applied; a caller
/// that needs to recover must clone the stage before calling [`run_pass`].
///
// The `Err` variant Quarantined object owns the whole stage by design. It
// makes the damaged IR inspectable but unusable. Boxing it, as clippy suggests,
// would not shrink the `Result` anyway: the `Ok` variant carries a `StageInfo` too.
#[allow(clippy::result_large_err)]
pub fn run_pass<L, F, T, E>(
    mut stage: StageInfo<L>,
    pass: F,
) -> Result<(StageInfo<L>, T), Quarantined<StageInfo<L>>>
where
    L: Dialect,
    E: std::error::Error + Send + Sync + 'static,
    F: FnOnce(&mut Rewriter<L>) -> Result<T, E>,
{
    let mut rewriter = Rewriter::new(&mut stage);

    let panic_or_result = catch_unwind(AssertUnwindSafe(|| pass(&mut rewriter)));
    let events = rewriter.drain_events();

    let pass_result = match panic_or_result {
        Ok(pass_result) => pass_result,
        Err(payload) => {
            return Err(Quarantined::from_stage(
                stage,
                QuarantineCause::from_panic(payload),
                events,
            ));
        }
    };

    // Check the pass did not fail
    let pass_output = match pass_result {
        Ok(output) => output,
        Err(error) => {
            return Err(Quarantined::from_stage(
                stage,
                QuarantineCause::Pass(Box::new(error)),
                events,
            ));
        }
    };

    // Verify the generated IR
    if let Err(error) = verify_derived(&stage) {
        return Err(Quarantined::from_stage(
            stage,
            QuarantineCause::Verify(error),
            events,
        ));
    }

    Ok((stage, pass_output))
}
