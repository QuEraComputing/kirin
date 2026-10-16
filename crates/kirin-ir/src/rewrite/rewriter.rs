//! Restricted in-place mutation layer over finalized IR.
//!
//! First slice of the rewrite engine described in
//! `docs/design/rewrite-engine.md`: a [`Rewriter`] that owns mutation of a
//! [`StageInfo`] and records every edit as a [`MutationEvent`].
//!
//! # What this layer does and does not guarantee
//!
//! Each method checks its own *executable* preconditions before writing, so a
//! rejected edit returns a [`RewriteError`] with the stage untouched. That is a
//! per-action property only — it is **not** a claim that the stage as a whole
//! stays valid. An accepted edit may leave the stage ill-typed, non-dominating,
//! or (see below) with desynchronized graph topology, because valid rewrites
//! often need several individually incomplete edits.
//!
//! Whole-stage usability is meant to be re-established at a *pass boundary*
//! that derives the expected metadata, compares it with what is installed, and
//! marks the stage unusable on failure — it never repairs. **That boundary does
//! not exist yet** (see M1 in the design doc): there is no `run_pass` ownership
//! scope and no `Quarantined` failure path. The comparison half is available
//! today as [`verify_derived`](crate::verify_derived), but nothing invokes it
//! automatically. Callers of this module get referential integrity, accurate
//! derived mirrors, and traversable block lists — nothing above that.
//!
//! Scope is operand/yield rewriting (including bulk result redirection through
//! [`Rewriter::replace_results`]), block-body statement surgery
//! ([`Rewriter::erase_statement`], [`Rewriter::insert_before`],
//! [`Rewriter::insert_after`], [`Rewriter::replace_statement`],
//! [`Rewriter::insert_terminator`]), and `CFG`
//! block-list surgery in the sibling `cfg` module (append/insert/erase/split/
//! merge blocks, [`Rewriter::retarget_successor`], and block-argument insertion
//! and erasure). The rewriter **maintains** all four derived mirrors
//! incrementally on every edit — [`SSAInfo::uses`](crate::SSAInfo),
//! [`BlockInfo::predecessors`](crate::BlockInfo), the block-body summary
//! (`BlockInfo::statements`/`terminator`), and the CFG chain summary
//! (`CFGInfo::blocks`) — so they stay valid without a rebuild. Deferred to
//! later slices: graph-body surgery, result-defining insertion, and full
//! cross-block dominance/visibility preflight.
//!
//! # Known gap: graph-body operand edits
//!
//! [`Rewriter::erase_statement`], [`Rewriter::replace_statement`], and the
//! `insert_*` methods reject `DiGraph`/`UnGraph`-owned statements with
//! [`RewriteError::NotInBlockBody`]. [`Rewriter::replace_operand`],
//! [`Rewriter::replace_all_uses`], and [`Rewriter::replace_results`] do **not**
//! check body kind: on a graph-owned statement they rewrite the operand and
//! leave the petgraph edge weights that mirror it stale. Until graph surgery
//! lands, prefer not to point them at graph bodies.
//!
//! Because value rewrites are index-driven (they consult and update
//! `SSAInfo::uses` rather than scanning), correctness depends on the use index
//! being accurate at entry — every mutation path must keep it in lockstep.
//! Inserting before a terminator likewise depends on `BlockInfo::statements`:
//! the terminator has no chain links, so its neighbour is read from the
//! chain's `tail`.
//! `BlockInfo::predecessors` is maintained differently: it is recomputed from
//! the statement's own `successors()`, so it does not depend on its own prior
//! accuracy. Direct arena writes that bypass the `Rewriter` desync the mirrors, and
//! `StageInfo::statement_arena_mut` is still public, so that bypass is
//! currently reachable.
//!
//! Surgery uses arena tombstones: an erased statement or block is marked
//! deleted (its id stays stable and resolves to `None`), never physically
//! removed. Erasure is eager: the statement-owned blocks and `CFG`s nested in
//! an erased statement or block are tombstoned with it. `DiGraph`/`UnGraph`
//! bodies are not tombstoned yet and survive their owner.

use std::collections::HashSet;
use std::fmt;

use crate::arena::GetInfo;
use crate::node::linked_list::LinkedListNode;
use smallvec::SmallVec;

use crate::{
    Block, BlockInfo, CFG, Dialect, SSAValue, StageInfo, Statement, StatementInfo, StatementParent,
    Use,
};

/// A structured record of one mutation performed through a [`Rewriter`].
///
/// Events are the typed replacement for a single "something changed" flag:
/// analyses can subscribe to the event kinds that invalidate them, and
/// worklist drivers can re-enqueue the statements named in the events.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum MutationEvent {
    /// One or more operand slots of `stmt` were rewritten in place.
    ChangedOperands { stmt: Statement },
    /// Every use of `old` was replaced by `new`.
    ///
    /// Emitted once per [`Rewriter::replace_all_uses`] call, after the
    /// per-statement [`MutationEvent::ChangedOperands`] events it implies.
    ReplacedUses { old: SSAValue, new: SSAValue },
    /// A new statement was spliced into a block's statement list.
    InsertedStatement { stmt: Statement },
    /// A statement was tombstoned and removed from its block.
    ///
    /// Covers everything nested in it: the statement-owned blocks and `CFG`s
    /// it held were tombstoned with it, and no separate
    /// [`MutationEvent::ChangedBlockList`] or [`MutationEvent::ChangedCfg`] is
    /// recorded for them.
    ErasedStatement { stmt: Statement },
    /// A statement's definition was swapped in place (same id and results).
    ReplacedStatement { stmt: Statement },
    /// Blocks were added to, removed from, or moved within `cfg`'s block list.
    ChangedBlockList { cfg: CFG },
    /// Control-flow edges or block arguments of blocks in `cfg` changed.
    ChangedCfg { cfg: CFG },
    /// `block`'s terminator was set (and with it the block's outgoing edges).
    ChangedTerminator { block: Block },
}

/// An illegal mutation rejected by a [`Rewriter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewriteError {
    /// The statement ID does not resolve to a live statement in this stage.
    UnknownStatement(Statement),
    /// The SSA value ID does not resolve to a live value in this stage.
    UnknownValue(SSAValue),
    /// The block ID does not resolve to a live block in this stage.
    UnknownBlock(Block),
    /// The operand index is out of range for the statement's operand list.
    OperandIndexOutOfRange { stmt: Statement, index: usize },
    /// The statement's parent is not a block body. Erasing/inserting relative to
    /// `DiGraph`/`UnGraph`-owned statements needs graph surgery, deferred to a
    /// later slice.
    NotInBlockBody(Statement),
    /// The statement defines results that are still used elsewhere, so erasing it
    /// would leave dangling uses. Replace those uses first.
    StatementResultsInUse(Statement),
    /// Nothing can follow a block's terminator, so `insert_after` cannot anchor
    /// on one. (`insert_before` can: it appends to the end of the chain.)
    AnchorIsTerminator(Statement),
    /// The inserted definition is a terminator; terminators are not spliced into
    /// the middle of a block's statement list. Use
    /// [`Rewriter::insert_terminator`] on a block that has none.
    CannotInsertTerminator,
    /// The inserted or replacement definition is a non-terminator that carries
    /// successors. Only a block's terminator may transfer control, so such a
    /// statement would add edges the predecessor mirror does not count.
    NonTerminatorWithSuccessors,
    /// `insert_terminator` was given a block that already has a terminator.
    BlockHasTerminator(Block),
    /// The inserted definition declares results; allocating fresh result SSA
    /// values (with types) is deferred to a later slice.
    CannotInsertWithResults,
    /// A result list did not line up one-for-one with the statement's results:
    /// a `replace_statement` definition changed the result count (which would
    /// orphan or invent result SSA values), or a `replace_results` call did not
    /// supply exactly one replacement per result.
    ResultArityMismatch {
        stmt: Statement,
        expected: usize,
        found: usize,
    },
    /// A `replace_statement` changed terminator-ness, which would desync the
    /// block's terminator cache. The replacement must match the original.
    TerminatorKindMismatch(Statement),
    /// The CFG ID does not resolve to a live CFG in this stage.
    UnknownCfg(CFG),
    /// The block is not part of a CFG's block list (it is a statement-owned
    /// single-block body, or detached), so block-list surgery cannot apply.
    NotInCfgBody(Block),
    /// The block belongs to a different CFG than the operation's CFG.
    CfgMismatch { block: Block, expected: CFG },
    /// `merge_blocks` was asked to merge a block into itself.
    CannotMergeBlockIntoItself(Block),
    /// The definition passed to `insert_terminator` is not a terminator.
    ExpectedTerminator,
    /// A list of values did not line up one-for-one with a block's arguments.
    BlockArgumentArityMismatch {
        block: Block,
        expected: usize,
        found: usize,
    },
    /// A block-argument index is out of range for the block's argument list
    /// (for insertion, past its end).
    BlockArgumentIndexOutOfRange { block: Block, index: usize },
    /// Dropping the block argument would leave dangling uses. Replace them first.
    BlockArgumentInUse { block: Block, index: usize },
    /// Removing `block` would leave `pred`'s edges into it dangling. Retarget
    /// them first.
    BlockHasPredecessors { block: Block, pred: Block },
    /// A `merge_blocks` replacement is one of the merged block's own
    /// arguments, which the merge deletes.
    ReplacementIsMergedArgument { block: Block, value: SSAValue },
    /// `stmt` branches to a block through successor position `successor`, but
    /// its [`BranchOp`](crate::BranchOp) gives no editable operand list for
    /// that edge.
    NoSuccessorOperands { stmt: Statement, successor: usize },
    /// The operand list `stmt` forwards on successor `successor` has no slot at
    /// `index` (for insertion, `index` is past its end).
    SuccessorOperandIndexOutOfRange {
        stmt: Statement,
        successor: usize,
        index: usize,
    },
}

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RewriteError::UnknownStatement(stmt) => {
                write!(
                    f,
                    "statement {stmt:?} is not a live statement in this stage"
                )
            }
            RewriteError::UnknownValue(value) => {
                write!(f, "SSA value {value} is not a live value in this stage")
            }
            RewriteError::UnknownBlock(block) => {
                write!(f, "block {block:?} is not a live block in this stage")
            }
            RewriteError::OperandIndexOutOfRange { stmt, index } => {
                write!(
                    f,
                    "operand index {index} is out of range for statement {stmt:?}"
                )
            }
            RewriteError::NotInBlockBody(stmt) => {
                write!(f, "statement {stmt:?} is not owned by a block body")
            }
            RewriteError::StatementResultsInUse(stmt) => {
                write!(
                    f,
                    "statement {stmt:?} has results that are still used; replace them first"
                )
            }
            RewriteError::AnchorIsTerminator(stmt) => {
                write!(f, "cannot insert after terminator {stmt:?}")
            }
            RewriteError::CannotInsertTerminator => {
                write!(
                    f,
                    "cannot splice a terminator into a block's statement list"
                )
            }
            RewriteError::BlockHasTerminator(block) => {
                write!(f, "block {block:?} already has a terminator")
            }
            RewriteError::CannotInsertWithResults => {
                write!(f, "cannot insert a statement that defines results yet")
            }
            RewriteError::ResultArityMismatch {
                stmt,
                expected,
                found,
            } => {
                write!(
                    f,
                    "replacement for {stmt:?} has {found} results, expected {expected}"
                )
            }
            RewriteError::TerminatorKindMismatch(stmt) => {
                write!(
                    f,
                    "replacement for {stmt:?} changes terminator-ness of the statement"
                )
            }
            RewriteError::UnknownCfg(cfg) => {
                write!(f, "CFG {cfg:?} is not a live CFG in this stage")
            }
            RewriteError::NotInCfgBody(block) => {
                write!(f, "block {block:?} is not part of a CFG block list")
            }
            RewriteError::CfgMismatch { block, expected } => {
                write!(f, "block {block:?} does not belong to CFG {expected:?}")
            }
            RewriteError::CannotMergeBlockIntoItself(block) => {
                write!(f, "cannot merge block {block:?} into itself")
            }
            RewriteError::ExpectedTerminator => {
                write!(f, "the definition must be a terminator")
            }
            RewriteError::NonTerminatorWithSuccessors => {
                write!(f, "only a terminator may carry successors")
            }
            RewriteError::BlockArgumentArityMismatch {
                block,
                expected,
                found,
            } => {
                write!(
                    f,
                    "block {block:?} has {expected} arguments, but {found} values were given"
                )
            }
            RewriteError::BlockArgumentIndexOutOfRange { block, index } => {
                write!(
                    f,
                    "argument index {index} is out of range for block {block:?}"
                )
            }
            RewriteError::NoSuccessorOperands { stmt, successor } => {
                write!(
                    f,
                    "{stmt:?} forwards no editable operands on successor {successor}"
                )
            }
            RewriteError::SuccessorOperandIndexOutOfRange {
                stmt,
                successor,
                index,
            } => {
                write!(
                    f,
                    "operand index {index} is out of range for successor {successor} of {stmt:?}"
                )
            }
            RewriteError::ReplacementIsMergedArgument { block, value } => {
                write!(
                    f,
                    "replacement {value} is an argument of {block:?}, which the merge deletes"
                )
            }
            RewriteError::BlockArgumentInUse { block, index } => {
                write!(
                    f,
                    "argument {index} of block {block:?} is still used; replace it first"
                )
            }
            RewriteError::BlockHasPredecessors { block, pred } => {
                write!(
                    f,
                    "block {pred:?} still branches to block {block:?}; retarget its edges first"
                )
            }
        }
    }
}

impl std::error::Error for RewriteError {}

/// The mutation entry point for finalized IR.
///
/// All rewrite edits flow through a `Rewriter` so redundant IR metadata
/// (use information, list links, caches) is maintained in one place and every
/// edit is recorded as a [`MutationEvent`].
pub struct Rewriter<'a, L: Dialect> {
    pub(super) stage: &'a mut StageInfo<L>,
    pub(super) events: Vec<MutationEvent>,
}

impl<'a, L: Dialect> Rewriter<'a, L> {
    pub fn new(stage: &'a mut StageInfo<L>) -> Self {
        Self {
            stage,
            events: Vec::new(),
        }
    }

    /// Read-only view of the stage being rewritten.
    pub fn stage(&self) -> &StageInfo<L> {
        self.stage
    }

    /// Events recorded so far, in application order.
    pub fn events(&self) -> &[MutationEvent] {
        &self.events
    }

    /// Take all recorded events, leaving the log empty.
    pub fn drain_events(&mut self) -> Vec<MutationEvent> {
        std::mem::take(&mut self.events)
    }

    /// The live statement `stmt`, or [`RewriteError::UnknownStatement`] for a
    /// deleted or unknown id.
    pub(super) fn live_statement(
        &self,
        stmt: Statement,
    ) -> Result<&StatementInfo<L>, RewriteError> {
        stmt.get_info(self.stage)
            .map(|item| &**item)
            .ok_or(RewriteError::UnknownStatement(stmt))
    }

    /// The live block `block`, or [`RewriteError::UnknownBlock`] for a
    /// deleted or unknown id.
    pub(super) fn live_block(&self, block: Block) -> Result<&BlockInfo<L>, RewriteError> {
        block
            .get_info(self.stage)
            .map(|item| &**item)
            .ok_or(RewriteError::UnknownBlock(block))
    }

    /// Rewrite the `index`-th operand of `stmt` to `value`, returning the
    /// previous operand.
    ///
    /// Maintains the def-use index: the `{stmt, index}` use moves from the
    /// previous operand's [`SSAInfo::uses`](crate::SSAInfo) to `value`'s.
    /// Writing the value already in the slot is a no-op and records no event.
    pub fn replace_operand(
        &mut self,
        stmt: Statement,
        index: usize,
        value: impl Into<SSAValue>,
    ) -> Result<SSAValue, RewriteError> {
        let value: SSAValue = value.into();
        if value.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownValue(value));
        }
        let item = stmt
            .get_info_mut(self.stage)
            .ok_or(RewriteError::UnknownStatement(stmt))?;
        let slot = item
            .definition
            .arguments_mut()
            .nth(index)
            .ok_or(RewriteError::OperandIndexOutOfRange { stmt, index })?;
        let old = *slot;
        if old == value {
            return Ok(old);
        }
        *slot = value;

        // Maintain the def-use index: this operand slot no longer reads `old`,
        // it now reads `value`.
        let site = Use::StatementOperand { stmt, index };
        remove_use(self.stage, old, site);
        add_use(self.stage, value, site);
        self.events.push(MutationEvent::ChangedOperands { stmt });
        Ok(old)
    }

    /// Replace every use of `old` with `new` across the stage, returning the
    /// number of use slots rewritten. Replacing a value with itself is a no-op
    /// returning `Ok(0)`.
    ///
    /// Index-driven: reads `old`'s [`SSAInfo::uses`](crate::SSAInfo) to visit
    /// exactly the slots that read `old` — statement operands **and `DiGraph`
    /// body yields** — rewrites each in place, then transfers those use records
    /// so `old` ends with an empty use list and `new` absorbs them. The
    /// def-use index stays valid; no rebuild is needed.
    ///
    /// Records one [`MutationEvent::ChangedOperands`] per statement whose
    /// operands changed, followed by a single [`MutationEvent::ReplacedUses`]
    /// if any slot was rewritten. (Yield-only rewrites are counted and covered
    /// by `ReplacedUses`, but have no dedicated granular event yet.)
    pub fn replace_all_uses(
        &mut self,
        old: impl Into<SSAValue>,
        new: impl Into<SSAValue>,
    ) -> Result<usize, RewriteError> {
        let old: SSAValue = old.into();
        let new: SSAValue = new.into();
        if old.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownValue(old));
        }
        if new.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownValue(new));
        }
        if old == new {
            return Ok(0);
        }
        Ok(self.redirect_uses(&[(old, new)]))
    }

    /// Redirect **every** result of `stmt` at once, one replacement per result
    /// in result order, exact arity required.
    ///
    /// This is the primitive multi-result rewrites need: CSE finds a duplicate
    /// statement, points all of its results at the original's results, then
    /// erases the duplicate. Hand-writing that as a loop of
    /// [`Rewriter::replace_all_uses`] calls over index-matched pairs is the
    /// error-prone path, so it must not be the convenient one.
    ///
    /// Arity is unchanged — this rewrites *uses* only, never the statement's own
    /// result list. Substitution is simultaneous, not sequential: every slot is
    /// rewritten against the use lists as they stand on entry, so naming one of
    /// `stmt`'s own results as a replacement redirects the two independently
    /// instead of chaining. A pair whose replacement is the result itself is a
    /// no-op.
    ///
    /// Rejects — leaving the stage untouched — if `stmt` is unknown, any
    /// replacement is not live, or `replacements` does not have exactly one
    /// entry per result. Records the same events as `replace_all_uses`: one
    /// [`MutationEvent::ChangedOperands`] per affected statement, then one
    /// [`MutationEvent::ReplacedUses`] per result that had uses to rewrite.
    pub fn replace_results(
        &mut self,
        stmt: Statement,
        replacements: &[SSAValue],
    ) -> Result<(), RewriteError> {
        let results: Vec<SSAValue> = {
            let item = self.live_statement(stmt)?;
            item.definition
                .results()
                .map(|r| SSAValue::from(*r))
                .collect()
        };
        if replacements.len() != results.len() {
            return Err(RewriteError::ResultArityMismatch {
                stmt,
                expected: results.len(),
                found: replacements.len(),
            });
        }
        for &new in replacements {
            if new.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(new));
            }
        }

        let pairs: Vec<(SSAValue, SSAValue)> = results
            .into_iter()
            .zip(replacements.iter().copied())
            .filter(|(old, new)| old != new)
            .collect();
        self.redirect_uses(&pairs);
        Ok(())
    }

    /// Simultaneously substitute each `old` with its paired `new` across every
    /// use slot, returning the number of slots rewritten.
    ///
    /// Every id must already be checked live and every pair must be
    /// non-trivial. Distinct values have disjoint use lists, so each slot is
    /// rewritten exactly once — and because the use lists are snapshotted
    /// before any write and cleared before any transfer, a value that is both
    /// replaced and a replacement ends up with exactly the sites redirected
    /// *to* it. A sequential loop would instead let a later pair carry away the
    /// sites an earlier one just moved.
    pub(super) fn redirect_uses(&mut self, pairs: &[(SSAValue, SSAValue)]) -> usize {
        // Snapshot the use sites so we can mutate the arenas while walking them
        // (the borrows of the `old` infos end here).
        let sites: Vec<Vec<Use>> = pairs
            .iter()
            .map(|(old, _)| {
                old.get_info(self.stage)
                    .map(|info| info.uses().to_vec())
                    .unwrap_or_default()
            })
            .collect();

        // Rewrite the slots, keeping the sites that actually applied — a stale
        // site names a statement or graph that has since been tombstoned.
        let mut changed_stmts: Vec<Statement> = Vec::new();
        let mut moved: Vec<Vec<Use>> = Vec::with_capacity(pairs.len());
        for (&(_, new), sites) in pairs.iter().zip(&sites) {
            let mut applied: Vec<Use> = Vec::with_capacity(sites.len());
            for &site in sites {
                let ok = match site {
                    Use::StatementOperand { stmt, index } => {
                        let ok = stmt
                            .get_info_mut(self.stage)
                            .and_then(|item| item.definition.arguments_mut().nth(index))
                            .map(|slot| *slot = new)
                            .is_some();
                        if ok && !changed_stmts.contains(&stmt) {
                            changed_stmts.push(stmt);
                        }
                        ok
                    }
                    Use::DiGraphYield { graph, index } => graph
                        .get_info_mut(self.stage)
                        .and_then(|item| item.yields_mut().get_mut(index))
                        .map(|slot| *slot = new)
                        .is_some(),
                };
                if ok {
                    applied.push(site);
                }
            }
            moved.push(applied);
        }

        // Transfer the use records in two phases: drop every moved list before
        // installing any of them, so a value appearing on both sides of the
        // substitution keeps only the sites redirected to it.
        for (&(old, _), moved) in pairs.iter().zip(&moved) {
            if !moved.is_empty()
                && let Some(info) = old.get_info_mut(self.stage)
            {
                info.uses_mut().clear();
            }
        }
        for (&(_, new), moved) in pairs.iter().zip(&moved) {
            if !moved.is_empty()
                && let Some(info) = new.get_info_mut(self.stage)
            {
                info.uses_mut().extend(moved.iter().copied());
            }
        }

        for stmt in changed_stmts {
            self.events.push(MutationEvent::ChangedOperands { stmt });
        }
        for (&(old, new), moved) in pairs.iter().zip(&moved) {
            if !moved.is_empty() {
                self.events.push(MutationEvent::ReplacedUses { old, new });
            }
        }
        moved.iter().map(Vec::len).sum()
    }

    /// Erase `stmt`: tombstone it, remove it from its block, drop its operand
    /// uses from the def-use index, and tombstone its result values. Erasing a
    /// block's terminator clears the block's terminator,
    /// removes its outgoing edges from the predecessor mirror, and leaves the
    /// block unterminated (see [`Rewriter::insert_terminator`]).
    /// Statement-owned blocks and `CFG`s nested inside `stmt` are tombstoned
    /// with it, recursively. `DiGraph`/`UnGraph` bodies are not tombstoned
    /// yet.
    ///
    /// Rejects — leaving the stage untouched — if `stmt` is unknown, is not
    /// owned by a block body, or still has results that are used elsewhere.
    /// Records [`MutationEvent::ErasedStatement`], followed by
    /// [`MutationEvent::ChangedTerminator`] when `stmt` was a terminator.
    pub fn erase_statement(&mut self, stmt: Statement) -> Result<(), RewriteError> {
        let (block, prev, next, operands, results, successors, is_terminator) = {
            let item = self.live_statement(stmt)?;
            let block = match item.parent {
                Some(StatementParent::Block(block)) => block,
                _ => return Err(RewriteError::NotInBlockBody(stmt)),
            };
            (
                block,
                item.node.prev,
                item.node.next,
                item.definition
                    .arguments()
                    .copied()
                    .collect::<Vec<SSAValue>>(),
                item.definition
                    .results()
                    .map(|r| SSAValue::from(*r))
                    .collect::<Vec<SSAValue>>(),
                item.definition
                    .successors()
                    .map(|successor| successor.target())
                    .collect::<HashSet<Block>>(),
                item.definition.is_terminator(),
            )
        };

        for &result in &results {
            let used = result
                .get_info(self.stage)
                .map(|info| !info.uses().is_empty())
                .unwrap_or(false);
            if used {
                return Err(RewriteError::StatementResultsInUse(stmt));
            }
        }

        if is_terminator {
            // The terminator is not in the chain; clear the block's pointer.
            if let Some(block_info) = block.get_info_mut(self.stage) {
                block_info.terminator = None;
            }
            // Only terminators carry successors, so the block's outgoing edges
            // are exactly this statement's; each target loses its entry.
            for target in successors {
                remove_predecessor(self.stage, target, block);
            }
        } else {
            // Unlink from the block's statement linked list (fix neighbours, or
            // the block head/tail when the statement is at an end).
            match prev {
                Some(p) => {
                    if let Some(info) = p.get_info_mut(self.stage) {
                        info.node.next = next;
                    }
                }
                None => {
                    if let Some(block_info) = block.get_info_mut(self.stage) {
                        block_info.statements.head = next;
                    }
                }
            }
            match next {
                Some(n) => {
                    if let Some(info) = n.get_info_mut(self.stage) {
                        info.node.prev = prev;
                    }
                }
                None => {
                    if let Some(block_info) = block.get_info_mut(self.stage) {
                        block_info.statements.tail = prev;
                    }
                }
            }
            if let Some(block_info) = block.get_info_mut(self.stage)
                && block_info.statements.len > 0
            {
                block_info.statements.len -= 1;
            }
        }

        // Drop the def-use edges this statement contributed, erase the bodies
        // nested in it, then tombstone its (now unused) result values and the
        // statement itself.
        for (index, operand) in operands.into_iter().enumerate() {
            remove_use(self.stage, operand, Use::StatementOperand { stmt, index });
        }
        self.tombstone_nested_bodies(stmt);
        for result in results {
            let _ = self.stage.ssas.delete(result);
        }
        let _ = self.stage.statement_arena_mut().delete(stmt);

        self.events.push(MutationEvent::ErasedStatement { stmt });
        if is_terminator {
            self.events.push(MutationEvent::ChangedTerminator { block });
        }
        Ok(())
    }

    /// Tombstone `block`, its arguments, and every statement it holds, the
    /// terminator included, recursing into nested bodies.
    ///
    /// Mirrors [`Rewriter::erase_statement`] per statement, minus its checks
    /// and chain relinking: the caller guarantees no value defined in `block`
    /// is used outside it, and the whole chain dies at once. Each statement's
    /// uses and outgoing edges are dropped, so the use and predecessor mirrors
    /// stay exact. Block-list links are left to the caller.
    pub(super) fn tombstone_block(&mut self, block: Block) {
        let Some(info) = block.get_info(self.stage) else {
            return;
        };
        let args: SmallVec<[SSAValue; 4]> = info.arguments.iter().map(|&a| a.into()).collect();
        let terminator = info.terminator;

        // A tombstoned statement keeps its links, so the chain can be walked
        // while it is being deleted.
        let mut current = info.statements.head;
        while let Some(stmt) = current {
            current = stmt.get_info(self.stage).and_then(|item| item.node.next);
            self.tombstone_statement(stmt, block);
        }
        if let Some(stmt) = terminator {
            self.tombstone_statement(stmt, block);
        }

        for arg in args {
            let _ = self.stage.ssas.delete(arg);
        }
        let _ = self.stage.nodes.blocks.delete(block);
    }

    /// The per-statement step of [`Rewriter::tombstone_block`]: drop `stmt`'s
    /// uses and its outgoing edges from `block`, erase its nested bodies, then
    /// tombstone its results and itself.
    fn tombstone_statement(&mut self, stmt: Statement, block: Block) {
        let (operands, results, successors) = {
            let definition = &stmt.expect_info(self.stage).definition;
            (
                definition
                    .arguments()
                    .copied()
                    .collect::<SmallVec<[SSAValue; 4]>>(),
                definition
                    .results()
                    .map(|r| SSAValue::from(*r))
                    .collect::<SmallVec<[SSAValue; 2]>>(),
                definition
                    .successors()
                    .map(|s| s.target())
                    .collect::<SmallVec<[Block; 2]>>(),
            )
        };
        for (index, operand) in operands.into_iter().enumerate() {
            remove_use(self.stage, operand, Use::StatementOperand { stmt, index });
        }
        for target in successors {
            remove_predecessor(self.stage, target, block);
        }
        self.tombstone_nested_bodies(stmt);
        for result in results {
            let _ = self.stage.ssas.delete(result);
        }
        let _ = self.stage.statement_arena_mut().delete(stmt);
    }

    /// Tombstone the statement-owned blocks and `CFG`s nested in `stmt`.
    ///
    /// TODO: `DiGraph`/`UnGraph` bodies are not tombstoned yet; their members
    /// survive their owner until graph surgery lands.
    fn tombstone_nested_bodies(&mut self, stmt: Statement) {
        let (blocks, cfgs) = {
            let definition = &stmt.expect_info(self.stage).definition;
            (
                definition
                    .blocks()
                    .copied()
                    .collect::<SmallVec<[Block; 2]>>(),
                definition.cfgs().copied().collect::<SmallVec<[CFG; 1]>>(),
            )
        };
        for block in blocks {
            self.tombstone_block(block);
        }
        for cfg in cfgs {
            let mut current = cfg.get_info(self.stage).and_then(|item| item.blocks.head);
            while let Some(block) = current {
                current = block.get_info(self.stage).and_then(|item| item.node.next);
                self.tombstone_block(block);
            }
            let _ = self.stage.nodes.cfgs.delete(cfg);
        }
    }

    /// Insert `definition` as a new statement immediately before `anchor`.
    ///
    /// Shares [`Rewriter::insert_after`]'s preconditions, except that `anchor`
    /// may be the block's terminator: the new statement is then appended to the
    /// end of the block's statement chain.
    pub fn insert_before(
        &mut self,
        anchor: Statement,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        self.insert_relative(anchor, definition, false)
    }

    /// Insert `definition` as a new statement immediately after `anchor`.
    ///
    /// `anchor` must be a live, non-terminator statement owned by a block body,
    /// and `definition` must be a non-terminator declaring no results (fresh
    /// result-value allocation is a later slice) and no successors. Every
    /// operand must be live. On success the new statement is spliced into the
    /// block, its operand uses are recorded, and
    /// [`MutationEvent::InsertedStatement`] is emitted; the new id is returned.
    pub fn insert_after(
        &mut self,
        anchor: Statement,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        self.insert_relative(anchor, definition, true)
    }

    fn insert_relative(
        &mut self,
        anchor: Statement,
        definition: L,
        after: bool,
    ) -> Result<Statement, RewriteError> {
        let (block, prev, next) = {
            let item = self.live_statement(anchor)?;
            let block = match item.parent {
                Some(StatementParent::Block(block)) => block,
                _ => return Err(RewriteError::NotInBlockBody(anchor)),
            };
            match (item.definition.is_terminator(), after) {
                // Nothing can follow a terminator.
                (true, true) => return Err(RewriteError::AnchorIsTerminator(anchor)),
                // The terminator sits outside the chain, so inserting before it
                // appends to the end of the chain
                (true, false) => {
                    let tail = block
                        .get_info(self.stage)
                        .and_then(|b| b.statements.tail().copied());

                    // The tail ends the chain, so the new statement becomes the tail.
                    debug_assert!(tail.is_none_or(|t| t.next(self.stage).is_none()));

                    (block, tail, None)
                }
                (false, true) => (block, Some(anchor), item.node.next),
                (false, false) => (block, item.node.prev, Some(anchor)),
            }
        };

        if definition.is_terminator() {
            return Err(RewriteError::CannotInsertTerminator);
        }
        if definition.successors().next().is_some() {
            return Err(RewriteError::NonTerminatorWithSuccessors);
        }
        if definition.results().next().is_some() {
            return Err(RewriteError::CannotInsertWithResults);
        }
        let operands: Vec<SSAValue> = definition.arguments().copied().collect();
        for &operand in &operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }

        let new_stmt = self
            .stage
            .statement_arena_mut()
            .alloc_with_id(|id| StatementInfo {
                node: LinkedListNode {
                    ptr: id,
                    next,
                    prev,
                },
                parent: Some(StatementParent::Block(block)),
                definition,
            });

        // Wire neighbours (or the block head/tail when inserting at an end).
        match prev {
            Some(p) => {
                if let Some(info) = p.get_info_mut(self.stage) {
                    info.node.next = Some(new_stmt);
                }
            }
            None => {
                if let Some(block_info) = block.get_info_mut(self.stage) {
                    block_info.statements.head = Some(new_stmt);
                }
            }
        }
        match next {
            Some(n) => {
                if let Some(info) = n.get_info_mut(self.stage) {
                    info.node.prev = Some(new_stmt);
                }
            }
            None => {
                if let Some(block_info) = block.get_info_mut(self.stage) {
                    block_info.statements.tail = Some(new_stmt);
                }
            }
        }
        if let Some(block_info) = block.get_info_mut(self.stage) {
            block_info.statements.len += 1;
        }

        for (index, operand) in operands.into_iter().enumerate() {
            add_use(
                self.stage,
                operand,
                Use::StatementOperand {
                    stmt: new_stmt,
                    index,
                },
            );
        }
        // Predecessor mirror does not need to be updated since we rejected terminators earlier.
        self.events
            .push(MutationEvent::InsertedStatement { stmt: new_stmt });
        Ok(new_stmt)
    }

    /// Insert `definition` as the terminator of `block`, which must have none
    /// (for example the first half left by [`Rewriter::split_block`]).
    ///
    /// The block may be in a `CFG` or a statement-owned body. Operand uses and
    /// the predecessor mirror are updated for the new statement's operands and
    /// successors, and the block's terminator pointer is set.
    ///
    /// Rejects, leaving the stage untouched, if `block` is unknown or already
    /// has a terminator, or if `definition` is not a terminator, declares
    /// results, or names a dead operand or successor.
    ///
    /// Records [`MutationEvent::InsertedStatement`] then
    /// [`MutationEvent::ChangedTerminator`].
    pub fn insert_terminator(
        &mut self,
        block: Block,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        {
            let info = self.live_block(block)?;
            if info.terminator.is_some() {
                return Err(RewriteError::BlockHasTerminator(block));
            }
        }
        if !definition.is_terminator() {
            return Err(RewriteError::ExpectedTerminator);
        }
        if definition.results().next().is_some() {
            return Err(RewriteError::CannotInsertWithResults);
        }
        let operands: Vec<SSAValue> = definition.arguments().copied().collect();
        for &operand in &operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }
        let targets: HashSet<Block> = definition.successors().map(|s| s.target()).collect();
        for &target in &targets {
            self.live_block(target)?;
        }

        let new_stmt = self
            .stage
            .statement_arena_mut()
            .alloc_with_id(|id| StatementInfo {
                node: LinkedListNode::new(id),
                parent: Some(StatementParent::Block(block)),
                definition,
            });
        for (index, operand) in operands.into_iter().enumerate() {
            let site = Use::StatementOperand {
                stmt: new_stmt,
                index,
            };
            add_use(self.stage, operand, site);
        }
        // Only terminators carry successors and the block had none, so every
        // target gains exactly one entry for `block`.
        for target in targets {
            add_predecessor(self.stage, target, block);
        }
        if let Some(info) = block.get_info_mut(self.stage) {
            info.terminator = Some(new_stmt);
        }

        self.events
            .push(MutationEvent::InsertedStatement { stmt: new_stmt });
        self.events.push(MutationEvent::ChangedTerminator { block });
        Ok(new_stmt)
    }

    /// Swap `stmt`'s definition for `definition` in place, keeping the same
    /// statement id, block position, and result values.
    ///
    /// The replacement must declare the same number of results and the same
    /// terminator-ness as the original, so result SSA values and the block's
    /// terminator cache stay valid; the original result ids are preserved
    /// regardless of what `definition` carries in its result slots.
    ///
    /// Every derived mirror stays current. Operand uses are updated to match
    /// the new operands, and `BlockInfo::predecessors` is updated for any
    /// control-flow edge the replacement adds or drops. The block-body and CFG
    /// chain mirrors need no update: the statement keeps its id, chain links,
    /// and terminator-ness, and no block moves.
    ///
    /// Rejects (leaving the stage untouched) if `stmt` is unknown, is not
    /// owned by a block body (i.e. in a graph), changes result arity or
    /// terminator-ness, gives a non-terminator successors, or names an operand
    /// or successor target that is not live.
    ///
    /// Records [`MutationEvent::ReplacedStatement`], followed by
    /// [`MutationEvent::ChangedTerminator`] when `stmt` is a terminator.
    pub fn replace_statement(
        &mut self,
        stmt: Statement,
        definition: L,
    ) -> Result<(), RewriteError> {
        let prepared = self.prepare_replacement(stmt, &definition)?;
        self.commit_replacement(stmt, prepared, definition);
        Ok(())
    }

    /// Check [`Rewriter::replace_statement`]'s preconditions without mutating,
    /// capturing the old state the commit step needs.
    pub(super) fn prepare_replacement(
        &self,
        stmt: Statement,
        definition: &L,
    ) -> Result<PreparedReplacement, RewriteError> {
        let (old_operands, old_results, old_successors, parent_block, old_is_terminator) = {
            let item = self.live_statement(stmt)?;
            let Some(StatementParent::Block(parent_block)) = item.parent else {
                return Err(RewriteError::NotInBlockBody(stmt));
            };
            (
                item.definition
                    .arguments()
                    .copied()
                    .collect::<Vec<SSAValue>>(),
                item.definition.results().copied().collect::<Vec<_>>(),
                item.definition
                    .successors()
                    .map(|successor| successor.target())
                    .collect::<HashSet<Block>>(),
                parent_block,
                item.definition.is_terminator(),
            )
        };

        let new_results = definition.results().count();
        if new_results != old_results.len() {
            return Err(RewriteError::ResultArityMismatch {
                stmt,
                expected: old_results.len(),
                found: new_results,
            });
        }
        if definition.is_terminator() != old_is_terminator {
            return Err(RewriteError::TerminatorKindMismatch(stmt));
        }
        if !old_is_terminator && definition.successors().next().is_some() {
            return Err(RewriteError::NonTerminatorWithSuccessors);
        }
        for &operand in definition.arguments() {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }
        for successor in definition.successors() {
            let target = successor.target();
            self.live_block(target)?;
        }

        Ok(PreparedReplacement {
            old_operands,
            old_results,
            old_successors,
            parent_block,
            is_terminator: old_is_terminator,
        })
    }

    /// Install a definition already checked by
    /// [`Rewriter::prepare_replacement`], keeping every derived mirror current
    /// (see [`Rewriter::replace_statement`]).
    pub(super) fn commit_replacement(
        &mut self,
        stmt: Statement,
        prepared: PreparedReplacement,
        definition: L,
    ) {
        let PreparedReplacement {
            old_operands,
            old_results,
            old_successors,
            parent_block,
            is_terminator,
        } = prepared;
        let new_operands: Vec<SSAValue> = definition.arguments().copied().collect();
        let new_successors: HashSet<Block> = definition
            .successors()
            .map(|successor| successor.target())
            .collect();

        // Preserve the original result ids in the incoming definition so its
        // results keep their SSA identity and types.
        let mut definition = definition;
        for (slot, old) in definition.results_mut().zip(old_results) {
            *slot = old;
        }

        // Update the def-use index: drop the old operand edges, install the new.
        for (index, operand) in old_operands.into_iter().enumerate() {
            remove_use(self.stage, operand, Use::StatementOperand { stmt, index });
        }
        if let Some(item) = stmt.get_info_mut(self.stage) {
            item.definition = definition;
        }
        for (index, operand) in new_operands.into_iter().enumerate() {
            add_use(self.stage, operand, Use::StatementOperand { stmt, index });
        }

        // Update the predecessor mirror. Only terminators are expected to have successors.
        // Remove the parent_block from the predecessor list of any successor that is
        // also not a successor of the new definition. Similarly, only add parent_block
        // to the predecessor list of any successor that does not already have it.
        // For example, when making the replacement cond_branch(t, t) -> cond_branch(t, u)
        // there is still an edge from parent_block to t, so we don't update t's predecessors
        if is_terminator {
            for &target in old_successors.difference(&new_successors) {
                remove_predecessor(self.stage, target, parent_block);
            }
            for &target in new_successors.difference(&old_successors) {
                add_predecessor(self.stage, target, parent_block);
            }
        }

        self.events.push(MutationEvent::ReplacedStatement { stmt });
        if is_terminator {
            self.events.push(MutationEvent::ChangedTerminator {
                block: parent_block,
            });
        }
    }
}

/// Old-state snapshot taken by [`Rewriter::prepare_replacement`].
pub(super) struct PreparedReplacement {
    old_operands: Vec<SSAValue>,
    old_results: Vec<crate::ResultValue>,
    old_successors: HashSet<Block>,
    parent_block: Block,
    is_terminator: bool,
}

/// Remove one occurrence of `site` from `value`'s use list, if present.
pub(super) fn remove_use<L: Dialect>(stage: &mut StageInfo<L>, value: SSAValue, site: Use) {
    if let Some(info) = value.get_info_mut(stage) {
        let uses = info.uses_mut();
        if let Some(pos) = uses.iter().position(|u| *u == site) {
            uses.swap_remove(pos);
        }
    }
}

/// Record `site` on `value`'s use list.
pub(super) fn add_use<L: Dialect>(stage: &mut StageInfo<L>, value: SSAValue, site: Use) {
    if let Some(info) = value.get_info_mut(stage) {
        info.uses_mut().push(site);
    }
}

/// Remove one occurrence of `pred` from `block`'s predecessor list, if present.
///
/// One entry per `(source, target)` pair, so callers must only reach this for
/// a target that does not have any other edges from `block`
pub(super) fn remove_predecessor<L: Dialect>(stage: &mut StageInfo<L>, block: Block, pred: Block) {
    let Some(block_info) = block.get_info_mut(stage) else {
        return;
    };
    if let Some(pos) = block_info.predecessors.iter().position(|p| *p == pred) {
        block_info.predecessors.swap_remove(pos);
    }
}

/// Record `pred` on `block`'s predecessor list.
///
/// One entry per `(source, target)` pair, so callers must only reach this for
/// a target that did not already have an edge from `pred`.
pub(super) fn add_predecessor<L: Dialect>(stage: &mut StageInfo<L>, block: Block, pred: Block) {
    if let Some(info) = block.get_info_mut(stage) {
        info.predecessors.push(pred);
    }
}
