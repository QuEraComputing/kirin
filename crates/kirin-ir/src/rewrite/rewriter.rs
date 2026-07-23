//! Safe mutation layer over finalized IR.
//!
//! First slice of the rewrite engine described in
//! `docs/design/rewrite-engine.md`: a [`Rewriter`] that owns mutation of a
//! [`StageInfo`], records every edit as a [`MutationEvent`], and rejects
//! illegal edits with a [`RewriteError`] instead of corrupting the arenas.
//!
//! Scope is operand/yield rewriting plus block-body statement surgery:
//! [`Rewriter::erase_statement`], [`Rewriter::insert_before`],
//! [`Rewriter::insert_after`], and [`Rewriter::replace_statement`]. The rewriter
//! **maintains** the def-use index ([`SSAInfo::uses`](crate::SSAInfo))
//! incrementally on every edit, so it stays valid without a rebuild. Deferred
//! to later slices: terminator/graph-body surgery, result-defining insertion,
//! and full cross-block dominance/visibility preflight.
//!
//! Because edits are index-driven (they consult and update `SSAInfo::uses`
//! rather than scanning), correctness depends on the index being accurate at
//! entry — every mutation path must keep it in lockstep. Direct arena writes
//! that bypass the `Rewriter` desync it.
//!
//! Statement surgery uses arena tombstones: an erased statement is marked
//! deleted (its id stays stable and resolves to `None`), never physically
//! removed. Every precondition is checked *before* any mutation, so a rejected
//! edit leaves the stage untouched.

use std::fmt;

use crate::arena::GetInfo;
use crate::node::linked_list::LinkedListNode;
use crate::{Dialect, SSAValue, StageInfo, Statement, StatementInfo, StatementParent, Use};

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
    /// A statement was tombstoned and unlinked from its block.
    ErasedStatement { stmt: Statement },
    /// A statement's definition was swapped in place (same id and results).
    ReplacedStatement { stmt: Statement },
}

/// An illegal mutation rejected by a [`Rewriter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewriteError {
    /// The statement ID does not resolve to a live statement in this stage.
    UnknownStatement(Statement),
    /// The SSA value ID does not resolve to a live value in this stage.
    UnknownValue(SSAValue),
    /// The operand index is out of range for the statement's operand list.
    OperandIndexOutOfRange { stmt: Statement, index: usize },
    /// The statement's parent is not a block body. Erasing/inserting relative to
    /// `DiGraph`/`UnGraph`-owned statements needs graph surgery, deferred to a
    /// later slice.
    NotInBlockBody(Statement),
    /// Erasing a block's terminator would leave the block ill-formed; this slice
    /// does not perform the required control-flow repair.
    CannotEraseTerminator(Statement),
    /// The statement defines results that are still used elsewhere, so erasing it
    /// would leave dangling uses. Replace those uses first.
    StatementResultsInUse(Statement),
    /// Insertion relative to a terminator is deferred (it would append to the
    /// non-terminator prefix or move past the terminator).
    AnchorIsTerminator(Statement),
    /// The inserted definition is a terminator; terminators are not spliced into
    /// the middle of a block's statement list.
    CannotInsertTerminator,
    /// The inserted definition declares results; allocating fresh result SSA
    /// values (with types) is deferred to a later slice.
    CannotInsertWithResults,
    /// A `replace_statement` changed the number of results, which would orphan or
    /// invent result SSA values. The replacement must have the same result arity.
    ResultArityMismatch {
        stmt: Statement,
        expected: usize,
        found: usize,
    },
    /// A `replace_statement` changed terminator-ness, which would desync the
    /// block's terminator cache. The replacement must match the original.
    TerminatorKindMismatch(Statement),
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
            RewriteError::OperandIndexOutOfRange { stmt, index } => {
                write!(
                    f,
                    "operand index {index} is out of range for statement {stmt:?}"
                )
            }
            RewriteError::NotInBlockBody(stmt) => {
                write!(f, "statement {stmt:?} is not owned by a block body")
            }
            RewriteError::CannotEraseTerminator(stmt) => {
                write!(f, "cannot erase block terminator {stmt:?}")
            }
            RewriteError::StatementResultsInUse(stmt) => {
                write!(
                    f,
                    "statement {stmt:?} has results that are still used; replace them first"
                )
            }
            RewriteError::AnchorIsTerminator(stmt) => {
                write!(f, "cannot insert relative to terminator {stmt:?}")
            }
            RewriteError::CannotInsertTerminator => {
                write!(
                    f,
                    "cannot splice a terminator into a block's statement list"
                )
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
    stage: &'a mut StageInfo<L>,
    events: Vec<MutationEvent>,
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

    /// Rewrite the `index`-th operand of `stmt` to `value`, returning the
    /// previous operand.
    ///
    /// Maintains the def-use index: the `{stmt, index}` use moves from the
    /// previous operand's [`SSAInfo::uses`](crate::SSAInfo) to `value`'s.
    /// Writing the value already in the slot is a no-op and records no event.
    pub fn set_operand(
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
            .filter(|item| !item.deleted())
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

        // Snapshot `old`'s use sites so we can mutate the arenas while walking
        // them (the borrow of `old`'s info ends here).
        let sites: Vec<Use> = old
            .get_info(self.stage)
            .map(|info| info.uses().to_vec())
            .unwrap_or_default();

        let mut moved: Vec<Use> = Vec::new();
        let mut changed_stmts: Vec<Statement> = Vec::new();
        for site in sites {
            let applied = match site {
                Use::StatementOperand { stmt, index } => {
                    let ok = stmt
                        .get_info_mut(self.stage)
                        .filter(|item| !item.deleted())
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
                    .filter(|item| !item.deleted())
                    .and_then(|item| item.yields_mut().get_mut(index))
                    .map(|slot| *slot = new)
                    .is_some(),
            };
            if applied {
                moved.push(site);
            }
        }

        let replaced = moved.len();
        if replaced > 0 {
            // Transfer the use records: `old` is no longer read anywhere; the
            // rewritten slots now read `new`.
            if let Some(info) = old.get_info_mut(self.stage) {
                info.uses_mut().clear();
            }
            if let Some(info) = new.get_info_mut(self.stage) {
                info.uses_mut().extend(moved);
            }
            for stmt in changed_stmts {
                self.events.push(MutationEvent::ChangedOperands { stmt });
            }
            self.events.push(MutationEvent::ReplacedUses { old, new });
        }
        Ok(replaced)
    }

    /// Erase `stmt`: tombstone it, unlink it from its block's statement list,
    /// drop its operand uses from the def-use index, and tombstone its result
    /// values.
    ///
    /// Rejects — leaving the stage untouched — if `stmt` is unknown, is not
    /// owned by a block body, is the block terminator, or still has results that
    /// are used elsewhere. Records [`MutationEvent::ErasedStatement`].
    pub fn erase_statement(&mut self, stmt: Statement) -> Result<(), RewriteError> {
        let (block, prev, next, operands, results, is_terminator) = {
            let item = stmt
                .get_info(self.stage)
                .filter(|item| !item.deleted())
                .ok_or(RewriteError::UnknownStatement(stmt))?;
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
                item.definition.is_terminator(),
            )
        };

        if is_terminator {
            return Err(RewriteError::CannotEraseTerminator(stmt));
        }
        for &result in &results {
            let used = result
                .get_info(self.stage)
                .map(|info| !info.uses().is_empty())
                .unwrap_or(false);
            if used {
                return Err(RewriteError::StatementResultsInUse(stmt));
            }
        }

        // Unlink from the block's statement linked list (fix neighbours, or the
        // block head/tail when the statement is at an end).
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

        // Drop the def-use edges this statement contributed, then tombstone its
        // (now unused) result values and the statement itself.
        for (index, operand) in operands.into_iter().enumerate() {
            remove_use(self.stage, operand, Use::StatementOperand { stmt, index });
        }
        for result in results {
            let _ = self.stage.ssas.delete(result);
        }
        let _ = self.stage.statement_arena_mut().delete(stmt);

        self.events.push(MutationEvent::ErasedStatement { stmt });
        Ok(())
    }

    /// Insert `definition` as a new statement immediately before `anchor`.
    /// Shares [`Rewriter::insert_after`]'s preconditions.
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
    /// result-value allocation is a later slice). Every operand must be live. On
    /// success the new statement is spliced into the block, its operand uses are
    /// recorded, and [`MutationEvent::InsertedStatement`] is emitted; the new id
    /// is returned.
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
        let (block, anchor_prev, anchor_next) = {
            let item = anchor
                .get_info(self.stage)
                .filter(|item| !item.deleted())
                .ok_or(RewriteError::UnknownStatement(anchor))?;
            let block = match item.parent {
                Some(StatementParent::Block(block)) => block,
                _ => return Err(RewriteError::NotInBlockBody(anchor)),
            };
            if item.definition.is_terminator() {
                return Err(RewriteError::AnchorIsTerminator(anchor));
            }
            (block, item.node.prev, item.node.next)
        };

        if definition.is_terminator() {
            return Err(RewriteError::CannotInsertTerminator);
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

        let (prev, next) = if after {
            (Some(anchor), anchor_next)
        } else {
            (anchor_prev, Some(anchor))
        };
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
        self.events
            .push(MutationEvent::InsertedStatement { stmt: new_stmt });
        Ok(new_stmt)
    }

    /// Swap `stmt`'s definition for `definition` in place, keeping the same
    /// statement id, block position, and result values.
    ///
    /// The replacement must declare the same number of results and the same
    /// terminator-ness as the original, so result SSA values and the block's
    /// terminator cache stay valid; the original result ids are preserved
    /// regardless of what `definition` carries in its result slots. Operand uses
    /// are updated to match the new operands. Records
    /// [`MutationEvent::ReplacedStatement`].
    pub fn replace_statement(
        &mut self,
        stmt: Statement,
        definition: L,
    ) -> Result<(), RewriteError> {
        let (old_operands, old_results, old_is_terminator) = {
            let item = stmt
                .get_info(self.stage)
                .filter(|item| !item.deleted())
                .ok_or(RewriteError::UnknownStatement(stmt))?;
            (
                item.definition
                    .arguments()
                    .copied()
                    .collect::<Vec<SSAValue>>(),
                item.definition.results().copied().collect::<Vec<_>>(),
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
        let new_operands: Vec<SSAValue> = definition.arguments().copied().collect();
        for &operand in &new_operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }

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
        self.events.push(MutationEvent::ReplacedStatement { stmt });
        Ok(())
    }
}

/// Remove one occurrence of `site` from `value`'s use list, if present.
fn remove_use<L: Dialect>(stage: &mut StageInfo<L>, value: SSAValue, site: Use) {
    if let Some(info) = value.get_info_mut(stage) {
        let uses = info.uses_mut();
        if let Some(pos) = uses.iter().position(|u| *u == site) {
            uses.swap_remove(pos);
        }
    }
}

/// Record `site` on `value`'s use list.
fn add_use<L: Dialect>(stage: &mut StageInfo<L>, value: SSAValue, site: Use) {
    if let Some(info) = value.get_info_mut(stage) {
        info.uses_mut().push(site);
    }
}
