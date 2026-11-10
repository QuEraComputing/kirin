use std::marker::PhantomData;

use kirin_ir::walk::{PreOrder, WalkOrder, WalkResult};
use kirin_ir::{Block, Dialect, GetInfo, Rewriter, StageInfo, Statement};

use super::{DriverError, Match, Worklist};
use crate::rule::{NativeRule, Rule};
use crate::{BlockRule, CheckedRule, RuleSet, StatementRule};

/// What a driver run did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RewriteSummary {
    /// Sweeps, each starting from every statement under the root, including
    /// the last one, which changed nothing.
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
/// A run is a fixpoint of sweeps. A sweep fills a worklist with the
/// statements under a root, in the walk order `O`:
/// [`PreOrder`](kirin_ir::walk::PreOrder) unless set with
/// [`walk_order`](Self::walk_order). The walk is a snapshot taken before any
/// rule runs. The sweep pops the statements one at a time, trying every rule
/// in order at each and applying each that matches.
///
/// After each statement, the edits its rules recorded as
/// [`MutationEvent`](kirin_ir::MutationEvent)s add to the worklist the
/// statements they may have given a new match: for each statement changed,
/// replaced or inserted, its owner, the producers of its operands, the
/// statement itself and the users of its results. A statement still waiting
/// keeps its place, so it is visited once, after the edit; one already
/// visited goes to the back. Other edits, such as erasing a statement or
/// changing a `CFG`, add nothing: the next sweep, which starts from every
/// statement again, sees them.
///
/// When the worklist is empty, the sweep erases the dead code under the
/// root. The run ends after the first sweep that records no
/// [`MutationEvent`](kirin_ir::MutationEvent); a rule that matches but edits
/// nothing does not count as a change.
///
/// Symbol attributes, the ones [`HasSymbols`](kirin_ir::HasSymbols) lists,
/// match by name through the pattern and program symbol tables, never by
/// their stage-local ids.
///
/// A rule's rewrite body may end in a walk result (see [`kirin_wildcard`]).
/// `interrupt` ends the sweep there, leaving the rest of the worklist
/// unvisited. `skip` has no effect: the sweep's walk is a snapshot, so it
/// cannot leave a statement's nested statements out. In an ordinary walk
/// (see [`kirin_ir::walk`]) it does.
///
/// An interrupted sweep counts like any other: if the run changed nothing,
/// it ends, without visiting the statements after the interrupt again. So a
/// rule should only `interrupt` after an application that edited the IR;
/// otherwise the run reports convergence although no rule in the set has
/// been tried on those statements. The driver does not check this.
///
/// Any rule may erase the statement being visited, including a pattern rule
/// erasing its root. The rules after it are not tried there.
///
/// A [`StatementRule`] is tried at every statement the sweep visits, in its
/// place in the rule list. It counts as applied when it edits the IR. Each
/// sweep, the driver builds a fresh copy of the rule with the recipe given
/// to [`RuleSet::add_statement_rule`]. The copy may be handed a statement
/// more than once in a sweep, when an edit adds it to the worklist again
/// after its visit.
///
/// A [`BlockRule`] runs, in its place in the rule list, on each block the
/// visited statement owns: its `Block`s, then the blocks of its `CFG`s.
/// Graph bodies are not handed to it. The blocks are listed when its turn
/// comes, so it sees the edits of the rules before it at that statement. A
/// statement visited again, such as the owner of a block whose statements an
/// edit changed, hands its blocks to the rule again. A listed block an earlier copy erased
/// or merged away is skipped. For every block, the driver builds a fresh
/// copy of the rule with the recipe given to [`RuleSet::add_block_rule`],
/// and each block whose run edits the IR counts as one application. An
/// `interrupt` ends the sweep at once, leaving the remaining blocks unrun.
///
/// Rules that keep undoing each other never reach a fixpoint. As safety
/// nets, a run still changing after [`max_sweeps`](Self::max_sweeps) sweeps
/// ([`DEFAULT_MAX_SWEEPS`](Self::DEFAULT_MAX_SWEEPS) unless set) fails with
/// [`DriverError::BudgetExhausted`], and a sweep making more than
/// [`max_rewrites_per_sweep`](Self::max_rewrites_per_sweep) rewrites
/// ([`DEFAULT_MAX_REWRITES_PER_SWEEP`](Self::DEFAULT_MAX_REWRITES_PER_SWEEP)
/// unless set) fails with [`DriverError::RewritesExhausted`], instead of
/// hanging. The limits are meant to be far above what a terminating rule
/// set needs, never a way to stop a run early.
///
/// The driver edits through the [`Rewriter`] a pass is given, so it runs
/// inside [`Pipeline::run_pass`](kirin_ir::Pipeline::run_pass):
///
/// ```ignore
/// pipeline.run_pass(stage, |rewriter| driver.run(rewriter))?;
/// ```
pub struct RewriteDriver<'r, L: Dialect, O: WalkOrder = PreOrder> {
    rules: &'r RuleSet<L>,
    max_sweeps: usize,
    max_rewrites_per_sweep: usize,
    order: PhantomData<O>,
}

impl<'r, L: Dialect> RewriteDriver<'r, L> {
    /// A driver applying `rules` in [`PreOrder`], with the default limits.
    pub fn new(rules: &'r RuleSet<L>) -> Self {
        Self {
            rules,
            max_sweeps: Self::DEFAULT_MAX_SWEEPS,
            max_rewrites_per_sweep: Self::DEFAULT_MAX_REWRITES_PER_SWEEP,
            order: PhantomData,
        }
    }
}

impl<'r, L: Dialect, O: WalkOrder> RewriteDriver<'r, L, O> {
    /// The sweep limit of a new driver.
    pub const DEFAULT_MAX_SWEEPS: usize = 10_000;

    /// The limit on rewrites in one sweep of a new driver.
    pub const DEFAULT_MAX_REWRITES_PER_SWEEP: usize = 10_000;

    /// Fill each sweep's worklist in the walk order `P` instead.
    pub fn walk_order<P: WalkOrder>(self) -> RewriteDriver<'r, L, P> {
        RewriteDriver {
            rules: self.rules,
            max_sweeps: self.max_sweeps,
            max_rewrites_per_sweep: self.max_rewrites_per_sweep,
            order: PhantomData,
        }
    }

    /// Fail a run on a root still changing after `max_sweeps` sweeps.
    pub fn max_sweeps(mut self, max_sweeps: usize) -> Self {
        self.max_sweeps = max_sweeps;
        self
    }

    /// Fail a run whose sweep makes more than `max_rewrites_per_sweep`
    /// rewrites.
    pub fn max_rewrites_per_sweep(mut self, max_rewrites_per_sweep: usize) -> Self {
        self.max_rewrites_per_sweep = max_rewrites_per_sweep;
        self
    }

    /// Run on every function in the stage: the definition of each live
    /// specialization, each to its own fixpoint, in turn.
    pub fn run(&self, rewriter: &mut Rewriter<'_, L>) -> Result<RewriteSummary, DriverError> {
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
            summary += self.run_at(rewriter, root)?;
        }
        Ok(summary)
    }

    /// Run on the tree under `root` until a sweep changes nothing.
    pub fn run_at(
        &self,
        rewriter: &mut Rewriter<'_, L>,
        root: Statement,
    ) -> Result<RewriteSummary, DriverError> {
        let mut summary = RewriteSummary::default();
        while summary.sweeps < self.max_sweeps {
            let applied = self.sweep(rewriter, root)?;
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

    /// Visit the statements under `root` from a worklist until it is empty,
    /// applying every rule that matches and every native rule, and adding
    /// what each visit's edits may affect. Returns how many applications
    /// edited the IR.
    ///
    /// An `interrupt` ends the sweep; a `skip` has no effect.
    fn sweep(&self, rewriter: &mut Rewriter<'_, L>, root: Statement) -> Result<usize, DriverError> {
        let mut applied = 0;
        let mut worklist = Worklist::seeded::<O, L>(rewriter.stage(), root);
        let mut sweep_rules: Vec<SweepRule<'_, L>> =
            self.rules.rules().iter().map(SweepRule::new).collect();
        while let Some(stmt) = worklist.pop() {
            // An earlier visit erased it.
            if stmt.get_info(rewriter.stage()).is_none() {
                continue;
            }
            let num_prior_events = rewriter.events().len();
            let walk_result = self.visit(&mut sweep_rules, rewriter, stmt, &mut applied)?;
            if applied > self.max_rewrites_per_sweep {
                return Err(DriverError::RewritesExhausted {
                    root,
                    rewrites: applied,
                });
            }
            if walk_result == WalkResult::Interrupt {
                break;
            }
            worklist.push_affected(rewriter.stage(), &rewriter.events()[num_prior_events..]);
        }
        Ok(applied)
    }

    /// Try `sweep_rules` at `stmt`, in order, adding to `applied` each
    /// application that edited the IR, and return how the walk goes on.
    ///
    /// After a `skip`, the remaining rules still run at `stmt`, and the
    /// result is `Skip`. An `interrupt` stops trying rules at once. A rule
    /// erasing `stmt` ends the visit, since no later rule can see it.
    fn visit(
        &self,
        sweep_rules: &mut [SweepRule<'r, L>],
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
        applied: &mut usize,
    ) -> Result<WalkResult, DriverError> {
        let mut walk_result = WalkResult::Advance;
        for rule in sweep_rules {
            match rule.apply(self.rules, rewriter, stmt, applied)? {
                WalkResult::Advance => {}
                WalkResult::Skip => walk_result = WalkResult::Skip,
                WalkResult::Interrupt => return Ok(WalkResult::Interrupt),
            }
            if stmt.get_info(rewriter.stage()).is_none() {
                break;
            }
        }
        Ok(walk_result)
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
