use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{Dialect, MutationEvent, QuarantineCause, Rewriter, StageInfo, verify_derived};

/// Run `pass` over a borrowed stage info and report what happened, taking
/// custody of nothing.
///
/// This is the mechanical half of a rewrite pass: lend the pass a [`Rewriter`],
/// contain an unwind, collect the mutation events, and check the derived
/// metadata afterwards. Holding only a borrow, it *cannot* build a
/// [`Quarantined`](crate::Quarantined). Choosing what to take custody of on failure belongs to
/// whoever owns the stage, namely, [`Pipeline::run_pass`](crate::Pipeline::run_pass).
///
/// The events come back on both paths: a failed pass needs them for its report,
/// and a successful one may still want to know what it changed.
///
/// # Failure routes
///
/// Three distinct failures produce three different [`QuarantineCause`]s:
///
/// - the pass returns `Err` — [`QuarantineCause::Pass`];
/// - the pass panics — [`QuarantineCause::Panic`], caught here rather than in
///   the pass so the stage survives the unwind and can still be inspected;
/// - the pass succeeds but leaves derived metadata disagreeing with the
///   authoritative IR — [`QuarantineCause::Verify`].
///
/// There is no rollback: edits made before a failure stay applied, which is why
/// the owner must take custody rather than hand the stage back.
pub(crate) fn drive_pass<L, F, T, E>(
    stage: &mut StageInfo<L>,
    pass: F,
) -> (Vec<MutationEvent>, Result<T, QuarantineCause>)
where
    L: Dialect,
    E: std::error::Error + Send + Sync + 'static,
    F: FnOnce(&mut Rewriter<L>) -> Result<T, E>,
{
    let mut rewriter = Rewriter::new(stage);

    let panic_or_result = catch_unwind(AssertUnwindSafe(|| pass(&mut rewriter)));
    let events = rewriter.drain_events();

    let pass_result = match panic_or_result {
        Ok(pass_result) => pass_result,
        Err(payload) => return (events, Err(QuarantineCause::from_panic(payload))),
    };

    // Check the pass did not fail
    let pass_output = match pass_result {
        Ok(output) => output,
        Err(error) => return (events, Err(QuarantineCause::Pass(Box::new(error)))),
    };

    // Verify the generated IR
    if let Err(error) = verify_derived(stage) {
        return (events, Err(QuarantineCause::Verify(error)));
    }

    (events, Ok(pass_output))
}
