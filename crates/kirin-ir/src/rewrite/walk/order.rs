use std::fmt;

use crate::arena::GetInfo;
use crate::{Block, Dialect, Rewriter, StageInfo, Statement, StatementParent};

/// A traversal order over the statements nested under a root statement.
///
/// Implemented by the marker types [`PreOrder`](super::PreOrder),
/// [`PostOrder`](super::PostOrder), [`BFS`](super::BFS) and
/// [`ReverseBFS`](super::ReverseBFS) and
/// [`KeepOrderReverseBFS`](super::KeepOrderReverseBFS), so a driver selects
/// its order as a type parameter.
#[expect(
    private_bounds,
    reason = "`CursorOrder` is crate-private so other crates cannot step a cursor by hand: \
              that would skip `walk_mut`'s checks of the walk contract"
)]
pub trait WalkOrder: CursorOrder {
    /// Whether an edit may erase the statement being visited, or move it by
    /// merging its block into another (see the [contract](super#contract)).
    /// Only the reverse BFS orders allow it: they build their order at the
    /// start, while the others find the next statement through the visited
    /// one.
    const ALLOWS_ERASING_VISITED: bool;

    /// Every statement of the walk under `root`, in order, for a walk that
    /// edits nothing. Empty if `root` is not live.
    fn schedule<L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Vec<Statement> {
        let mut cursor = Self::start(stage, root);
        let mut ordered_nodes = Vec::new();
        while let Some(stmt) = cursor.next(stage) {
            ordered_nodes.push(stmt);
        }
        ordered_nodes
    }

    /// Walk the tree under `root`, calling `visit` at each statement and
    /// steering the walk by what it returns. `visit` cannot edit, so the
    /// walk cannot fail; for one that edits, use [`walk_mut`](Self::walk_mut).
    fn walk<L: Dialect>(
        stage: &StageInfo<L>,
        root: Statement,
        mut visit: impl FnMut(&StageInfo<L>, Statement) -> WalkResult,
    ) -> WalkStatus {
        let mut cursor = Self::start(stage, root);
        while let Some(stmt) = cursor.next(stage) {
            if let Some(status) = visit(stage, stmt).steer(&mut cursor) {
                return status;
            }
        }
        WalkStatus::Completed
    }

    /// [`walk`](Self::walk) with a `visit` that may edit through `rewriter`,
    /// within the walk's contract (see the [module docs](super)). Fails if
    /// `visit` erases the statement it is visiting where the order does not
    /// allow it, or moves it by merging its block into another, whatever
    /// `visit` returns: an interrupted walk is checked too.
    fn walk_mut<'r, L: Dialect>(
        rewriter: &mut Rewriter<'r, L>,
        root: Statement,
        mut visit: impl FnMut(&mut Rewriter<'r, L>, Statement) -> WalkResult,
    ) -> Result<WalkStatus, WalkError> {
        let mut cursor = Self::start(rewriter.stage(), root);
        while let Some(stmt) = cursor.next(rewriter.stage()) {
            let block = block_of(rewriter.stage(), stmt);
            let result = visit(rewriter, stmt);
            // Check the contract before stepping on, so the cursor never steps
            // from a statement erased or moved where the order forbids it.
            cursor.check_erasure(rewriter.stage())?;
            if !Self::ALLOWS_ERASING_VISITED {
                check_not_merged_away(rewriter.stage(), stmt, block)?;
            }
            // `None` on `Advance` and `Skip`, the final status on `Interrupt`.
            if let Some(status) = result.steer(&mut cursor) {
                return Ok(status);
            }
        }
        Ok(WalkStatus::Completed)
    }
}

/// Fail with [`WalkError::VisitedMoved`] if `visited` is live but `block`,
/// the block it was in when its visit started, is not: the visit merged
/// `block` into another block. Only a merge erases a block and keeps its
/// statements; a split keeps the original block live, so it passes. An
/// erased `visited` passes: erasure is [`WalkCursor::check_erasure`]'s to
/// judge.
fn check_not_merged_away<L: Dialect>(
    stage: &StageInfo<L>,
    visited: Statement,
    block: Option<Block>,
) -> Result<(), WalkError> {
    let merged = visited.get_info(stage).is_some()
        && block.is_some_and(|block| block.get_info(stage).is_none());
    if merged {
        Err(WalkError::VisitedMoved(visited))
    } else {
        Ok(())
    }
}

/// The block `stmt` is in, if it is in one.
fn block_of<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Option<Block> {
    match stmt.get_info(stage)?.parent? {
        StatementParent::Block(block) => Some(block),
        StatementParent::DiGraph(_) | StatementParent::UnGraph(_) => None,
    }
}

/// What a walk does after visiting a statement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WalkResult {
    /// Go on to the next statement.
    #[default]
    Advance,
    /// Go on, without walking the statements nested in this one. Has no
    /// effect in an order that visits a statement after the statements
    /// nested in it ([`PostOrder`](super::PostOrder),
    /// [`ReverseBFS`](super::ReverseBFS),
    /// [`KeepOrderReverseBFS`](super::KeepOrderReverseBFS)).
    Skip,
    /// End the walk.
    Interrupt,
}

impl WalkResult {
    /// Apply `self` to `cursor`: `Some` to end the walk with that status.
    fn steer(self, cursor: &mut impl WalkCursor) -> Option<WalkStatus> {
        match self {
            WalkResult::Advance => None,
            WalkResult::Skip => {
                cursor.skip();
                None
            }
            WalkResult::Interrupt => Some(WalkStatus::Interrupted),
        }
    }
}

/// How a walk ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WalkStatus {
    /// Every statement of the walk was visited.
    Completed,
    /// A visit returned [`WalkResult::Interrupt`].
    Interrupted,
}

/// The cursor an order walks with, and how to start one at a root.
///
/// Crate-private, so that outside this crate a walk is only
/// [`WalkOrder::walk`], [`WalkOrder::walk_mut`] or [`WalkOrder::schedule`]:
/// an editing walk then always goes through `walk_mut`, which checks the
/// whole contract, while a cursor stepped by hand checks none of it.
pub(crate) trait CursorOrder {
    /// The cursor walking this order.
    type Cursor: WalkCursor;

    /// A cursor at the start of the walk of the tree under `root`, `root`
    /// included.
    fn start<L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Self::Cursor;
}

/// A position in a walk. It holds only ids, so the IR can be edited between
/// steps.
pub(crate) trait WalkCursor {
    /// The next statement to visit, or `None` once the walk is done.
    ///
    /// The IR may be edited between calls within the walk's contract (see
    /// the [module docs](super)). The cursor does not check that contract:
    /// [`WalkOrder::walk_mut`], the only walk that edits, checks it after
    /// each visit, before stepping on.
    fn next<L: Dialect>(&mut self, stage: &StageInfo<L>) -> Option<Statement>;

    /// Fail if the statement returned by the previous [`next`](Self::next)
    /// has since been erased where the order does not allow it. Only reads;
    /// the cursor does not move.
    fn check_erasure<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<(), WalkError>;

    /// Do not walk the statements nested in the statement the previous
    /// [`next`](Self::next) returned, including any added while visiting it.
    /// Has no effect before the first step, after the last, or in an order
    /// that visits a statement after the statements nested in it
    /// ([`PostOrder`](super::PostOrder), [`ReverseBFS`](super::ReverseBFS),
    /// [`KeepOrderReverseBFS`](super::KeepOrderReverseBFS)).
    fn skip(&mut self);
}

/// A walk whose contract was broken by an edit made during it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkError {
    /// The statement being visited was erased before the walk proceeded, in a
    /// way its order does not allow (see the [module docs](super)).
    VisitedErased(Statement),
    /// The statement being visited was moved before the walk proceeded, by
    /// merging its block into another block
    /// ([`Rewriter::merge_blocks`](crate::Rewriter::merge_blocks) with its
    /// block as `succ`).
    VisitedMoved(Statement),
}

impl fmt::Display for WalkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalkError::VisitedErased(stmt) => {
                write!(
                    f,
                    "statement {stmt:?} was erased while the walk was visiting it"
                )
            }
            WalkError::VisitedMoved(stmt) => {
                write!(
                    f,
                    "statement {stmt:?} was moved into another block while the walk was visiting it"
                )
            }
        }
    }
}

impl std::error::Error for WalkError {}

/// Where a cursor is: before the first statement, at one, or past the last.
#[derive(Clone, Copy, Debug)]
pub(super) enum Position {
    Start,
    At(Statement),
    /// At a statement whose nested statements are not walked.
    Skipped(Statement),
    Done,
}

impl Position {
    /// The statement the cursor is at, if any.
    pub(super) fn visited(self) -> Option<Statement> {
        match self {
            Position::At(stmt) | Position::Skipped(stmt) => Some(stmt),
            Position::Start | Position::Done => None,
        }
    }

    /// Mark the statement the cursor is at as skipped; nothing to mark
    /// before the first statement or past the last.
    pub(super) fn skip(&mut self) {
        if let Position::At(stmt) = *self {
            *self = Position::Skipped(stmt);
        }
    }
}
