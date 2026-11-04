use kirin_ir::walk::{WalkCursor, WalkOrder, WalkResult};
use kirin_ir::{Block, Dialect, GetInfo, Rewriter, StageInfo, Statement};

use super::{DriverError, Match};
use crate::rule::{NativeRule, Rule};
use crate::{BlockRule, CheckedRule, RuleSet, StatementRule};

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
/// Since a pattern's root never owns bodies yet (see
/// [`RuleErrorKind::OpWithBodies`](crate::RuleErrorKind::OpWithBodies)),
/// `skip` from a pattern rule has no effect so far.
///
/// A [`StatementRule`] is tried at every statement the sweep visits, in its
/// place in the rule list, and steers the sweep with the walk result it
/// returns. It counts as applied when it edits the IR. Each sweep, the
/// driver builds a fresh copy of the rule with the recipe given to
/// [`RuleSet::add_statement_rule`]. Since its edits can't be known before the
/// run, one erasing the statement being visited in an order that does not
/// allow it fails the run with [`DriverError::Walk`].
///
/// A [`BlockRule`] runs, in its place in the rule list, on each block the
/// visited statement owns: its `Block`s, then the blocks of its `CFG`s.
/// Graph bodies are not handed to it. The blocks are listed when its turn
/// comes, so it sees the edits of the rules before it at that statement. A
/// block is therefore handed to it this sweep only if it is under its owner
/// when the walk visits the owner: one created or moved under a statement
/// the walk has already passed waits for the next sweep, which the edit
/// causes. A listed block an earlier copy erased or merged away is skipped.
/// For every block, the driver builds a fresh copy of the rule with the
/// recipe given to [`RuleSet::add_block_rule`], and each block whose run
/// edits the IR counts as one application. A `skip` from one block still
/// runs the rule on the statement's remaining blocks, such as an `scf.if`'s
/// `else` arm after its `then` arm, then leaves the statement's nested
/// statements unwalked. An `interrupt` ends the sweep at once, leaving the
/// remaining blocks unrun.
///
/// An interrupted sweep counts like any other: if the run changed nothing,
/// it ends, without visiting the statements after the interrupt again. So a
/// rule should only `interrupt` after an application that edited the IR;
/// otherwise the run reports convergence although no rule in the set has
/// been tried on those statements. The driver does not check this.
///
/// Rules that keep undoing each other never reach a fixpoint. As a safety
/// net, a run still changing after [`max_sweeps`](Self::max_sweeps) sweeps
/// ([`DEFAULT_MAX_SWEEPS`](Self::DEFAULT_MAX_SWEEPS) unless set) fails with
/// [`DriverError::BudgetExhausted`] instead of hanging. The limit is meant to
/// be far above what a terminating rule set needs, never a way to stop a
/// run early. A single sweep has no limit.
///
/// A rule erasing its root erases the statement being visited, which only
/// some walk orders allow (see [`kirin_ir::walk`]). A run in another order
/// fails with [`DriverError::ErasesRoot`] before editing anything. Native
/// actions can edit anything, so they are checked by the walk as it runs: one
/// breaking the walk's contract fails the run with [`DriverError::Walk`].
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
        if !O::ALLOWS_ERASING_VISITED
            && let Some(rule) = self
                .rules
                .rules()
                .iter()
                .filter_map(Rule::as_pattern)
                .find(|rule| rule.erases_root())
        {
            let order = std::any::type_name::<O>();
            return Err(DriverError::ErasesRoot {
                rule: rule.name().to_string(),
                order: order.rsplit("::").next().unwrap_or(order),
            });
        }
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

    /// Walk the tree under `root` once, applying every rule that matches
    /// and every native rule. Returns how many applications edited the IR.
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
        let mut sweep_rules: Vec<SweepRule<'_, L>> =
            self.rules.rules().iter().map(SweepRule::new).collect();
        while let Some(stmt) = cursor.next(rewriter.stage())? {
            let mut skip = false;
            for rule in &mut sweep_rules {
                match rule.apply(self.rules, rewriter, stmt, &mut applied)? {
                    WalkResult::Advance => {}
                    WalkResult::Skip => skip = true,
                    WalkResult::Interrupt => return Ok(applied),
                }
                // The rule erased the statement, so no later rule can see it.
                if stmt.get_info(rewriter.stage()).is_none() {
                    break;
                }
            }
            if skip {
                cursor.skip();
            }
        }
        Ok(applied)
    }
}

/// A rule as one sweep holds it. Native rules of both kinds get fresh
/// copies, at different times: a statement rule one per sweep, so the sweep
/// builds it at the start and keeps it; a block rule one per block, so the
/// sweep keeps its recipe and builds a copy at each block.
enum SweepRule<'r, L: Dialect> {
    Pattern(&'r CheckedRule<L>),
    Statement {
        name: &'r str,
        rule: Box<dyn StatementRule<L>>,
    },
    Block(&'r NativeRule<dyn BlockRule<L>>),
}

impl<'r, L: Dialect> SweepRule<'r, L> {
    fn new(rule: &'r Rule<L>) -> Self {
        match rule {
            Rule::Pattern(rule) => SweepRule::Pattern(rule),
            Rule::Statement(native) => SweepRule::Statement {
                name: native.name(),
                rule: native.build(),
            },
            Rule::Block(native) => SweepRule::Block(native),
        }
    }

    /// Run the rule at `stmt`, adding to `applied` each application that
    /// edited the IR, and return how the walk goes on.
    ///
    /// A block rule runs on each block `stmt` owns, listed when its turn
    /// comes, so after the rules before it ran. A `skip` from one block
    /// still runs it on `stmt`'s remaining sibling blocks; an `interrupt`
    /// stops at once.
    fn apply(
        &mut self,
        rules: &RuleSet<L>,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
        applied: &mut usize,
    ) -> Result<WalkResult, DriverError> {
        let failed = |name: &str, error| DriverError::Rewrite {
            rule: name.to_string(),
            stmt,
            error,
        };
        let before = rewriter.events().len();
        let walk_result = match self {
            SweepRule::Pattern(rule) => {
                let Some(found) = Match::find(rules, rule, rewriter.stage(), stmt)? else {
                    return Ok(WalkResult::Advance);
                };
                found.apply(rules, rule, rewriter, stmt)?
            }
            SweepRule::Statement { name, rule } => rule
                .apply(rewriter, stmt)
                .map_err(|error| failed(name, error))?,
            SweepRule::Block(native) => {
                let mut walk_result = WalkResult::Advance;
                for block in owned_blocks(rewriter.stage(), stmt) {
                    // An earlier copy erased or merged away this block.
                    if block.get_info(rewriter.stage()).is_none() {
                        continue;
                    }
                    let before = rewriter.events().len();
                    let result = native
                        .build()
                        .apply(rewriter, block)
                        .map_err(|error| failed(native.name(), error))?;
                    if rewriter.events().len() > before {
                        *applied += 1;
                    }
                    match result {
                        WalkResult::Advance => {}
                        WalkResult::Skip => walk_result = WalkResult::Skip,
                        WalkResult::Interrupt => return Ok(WalkResult::Interrupt),
                    }
                }
                return Ok(walk_result);
            }
        };
        if rewriter.events().len() > before {
            *applied += 1;
        }
        Ok(walk_result)
    }
}

/// The blocks `stmt` owns: its `Block`s, then the blocks of its `CFG`s in
/// layout order. Empty if `stmt` is not live.
pub(super) fn owned_blocks<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Vec<Block> {
    let Some(info) = stmt.get_info(stage) else {
        return Vec::new();
    };
    let definition = info.definition();
    definition
        .blocks()
        .copied()
        .chain(definition.cfgs().flat_map(|cfg| cfg.blocks(stage)))
        .collect()
}
