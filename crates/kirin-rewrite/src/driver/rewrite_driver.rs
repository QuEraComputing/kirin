use kirin_ir::walk::{WalkCursor, WalkOrder, WalkResult};
use kirin_ir::{Dialect, Rewriter, Statement};

use super::{DriverError, Match};
use crate::RuleSet;

/// What a driver run did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RewriteSummary {
    /// Walks over the IR, including the last one, which changed nothing.
    pub sweeps: usize,
    /// Rule applications that edited the IR.
    pub applied: usize,
    /// Statements erased by the dead-code cleanup after each sweep.
    pub erased: usize,
}

impl std::ops::AddAssign for RewriteSummary {
    fn add_assign(&mut self, other: Self) {
        self.sweeps += other.sweeps;
        self.applied += other.applied;
        self.erased += other.erased;
    }
}

/// Applies a [`RuleSet`] to IR until nothing changes.
///
/// A run is a fixpoint of sweeps. A sweep walks the tree under a root in a
/// [`WalkOrder`] chosen per call, such as
/// [`PostOrder`](kirin_ir::walk::PostOrder), trying every rule in order at
/// each statement and applying each that matches. It then erases the dead
/// code under the root. The run ends after the first sweep that records no
/// [`MutationEvent`](kirin_ir::MutationEvent); a rule that matches but edits
/// nothing does not count as a change.
///
/// Symbol attributes, the ones [`HasSymbols`](kirin_ir::HasSymbols) lists,
/// match by name through the pattern and program symbol tables, never by
/// their stage-local ids.
///
/// A rule's rewrite body may end in a walk result steering the sweep (see
/// [`kirin_wildcard`]): `skip` leaves the statements nested in the
/// statement being visited unwalked, and `interrupt` ends the sweep there.
/// An interrupted sweep counts like any other: if nothing changed, the run
/// ends, without visiting the statements after the interrupt again. Since a
/// rule's root never owns bodies yet (see
/// [`RuleErrorKind::OpWithBodies`](crate::RuleErrorKind::OpWithBodies)),
/// `skip` has no effect so far.
///
/// Rules that keep undoing each other never reach a fixpoint. As a safety
/// net, a run still changing after [`max_sweeps`](Self::max_sweeps) sweeps
/// ([`DEFAULT_MAX_SWEEPS`](Self::DEFAULT_MAX_SWEEPS) unless set) fails with
/// [`DriverError::BudgetExhausted`] instead of hanging. The limit is meant to
/// be far above what a terminating rule set needs, never a way to stop a
/// run early. A single sweep has no limit.
///
/// Rules never erase the statement being visited, which
/// [`RuleSet::add`](crate::RuleSet::add) checks, so dead statements are left
/// to the dead-code cleanup. Native actions are not checked; one breaking the
/// walk's contract (see [`kirin_ir::walk`]) fails the run.
///
/// The driver edits through the [`Rewriter`] a pass is given, so it runs
/// inside [`Pipeline::run_pass`](kirin_ir::Pipeline::run_pass):
///
/// ```ignore
/// pipeline.run_pass(stage, |rewriter| driver.run::<PostOrder>(rewriter))?;
/// ```
pub struct RewriteDriver<'r, L: Dialect> {
    rules: &'r RuleSet<L>,
    max_sweeps: usize,
}

impl<'r, L: Dialect> RewriteDriver<'r, L> {
    /// The sweep limit of a new driver.
    pub const DEFAULT_MAX_SWEEPS: usize = 10_000;

    pub fn new(rules: &'r RuleSet<L>) -> Self {
        Self {
            rules,
            max_sweeps: Self::DEFAULT_MAX_SWEEPS,
        }
    }

    /// Fail a run on a root still changing after `max_sweeps` sweeps.
    pub fn max_sweeps(mut self, max_sweeps: usize) -> Self {
        self.max_sweeps = max_sweeps;
        self
    }

    /// Run on every function in the stage: the definition of each live
    /// specialization, each to its own fixpoint, in turn.
    pub fn run<O: WalkOrder>(
        &self,
        rewriter: &mut Rewriter<'_, L>,
    ) -> Result<RewriteSummary, DriverError> {
        let roots: Vec<Statement> = rewriter
            .stage()
            .staged_function_arena()
            .iter()
            .flat_map(|function| function.specializations())
            .filter(|specialization| !specialization.is_invalidated())
            .map(|specialization| *specialization.definition())
            .collect();
        let mut summary = RewriteSummary::default();
        for root in roots {
            summary += self.run_at::<O>(rewriter, root)?;
        }
        Ok(summary)
    }

    /// Run on the tree under `root` until a sweep changes nothing.
    pub fn run_at<O: WalkOrder>(
        &self,
        rewriter: &mut Rewriter<'_, L>,
        root: Statement,
    ) -> Result<RewriteSummary, DriverError> {
        let mut summary = RewriteSummary::default();
        while summary.sweeps < self.max_sweeps {
            let applied = self.sweep::<O>(rewriter, root)?;
            let erased = Self::clean_up_dead_code(rewriter, root)?;
            summary += RewriteSummary {
                sweeps: 1,
                applied,
                erased,
            };
            if applied == 0 && erased == 0 {
                return Ok(summary);
            }
        }
        Err(DriverError::BudgetExhausted {
            root,
            sweeps: summary.sweeps,
        })
    }

    /// Walk the tree under `root` once, applying every rule that matches.
    /// Returns how many applications edited the IR.
    ///
    /// Each applied rule's walk result steers the walk: after a `skip`, the
    /// remaining rules still run at the statement, then its nested
    /// statements are skipped; an `interrupt` stops trying rules and ends
    /// the sweep.
    fn sweep<O: WalkOrder>(
        &self,
        rewriter: &mut Rewriter<'_, L>,
        root: Statement,
    ) -> Result<usize, DriverError> {
        let mut applied = 0;
        let mut cursor = O::start(rewriter.stage(), root);
        while let Some(stmt) = cursor.next(rewriter.stage())? {
            let mut skip = false;
            for rule in self.rules.rules() {
                let Some(found) = Match::find(self.rules, rule, rewriter.stage(), stmt)? else {
                    continue;
                };
                let before = rewriter.events().len();
                let walk_result = found.apply(self.rules, rule, rewriter, stmt)?;
                if rewriter.events().len() > before {
                    applied += 1;
                }
                match walk_result {
                    WalkResult::Advance => {}
                    WalkResult::Skip => skip = true,
                    WalkResult::Interrupt => return Ok(applied),
                }
            }
            if skip {
                cursor.skip();
            }
        }
        Ok(applied)
    }
}
