use std::fmt;

use crate::{Dialect, Rewriter, StageInfo, Statement};

/// A traversal order over the statements nested under a root statement.
///
/// Implemented by the marker types [`PreOrder`](super::PreOrder),
/// [`PostOrder`](super::PostOrder), [`BFS`](super::BFS) and
/// [`ReverseBFS`](super::ReverseBFS) and
/// [`KeepOrderReverseBFS`](super::KeepOrderReverseBFS), so a driver selects
/// its order as a type parameter.
pub trait WalkOrder {
    /// The cursor walking this order.
    type Cursor: WalkCursor;

    /// A cursor at the start of the walk of the tree under `root`, `root`
    /// included.
    fn start<L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Self::Cursor;

    /// Every statement of the walk under `root`, in order, for a walk that
    /// edits nothing. Empty if `root` is not live.
    fn schedule<L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Vec<Statement> {
        let mut cursor = Self::start(stage, root);
        let mut ordered_nodes = Vec::new();
        while let Ok(Some(stmt)) = cursor.next(stage) {
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
        while let Ok(Some(stmt)) = cursor.next(stage) {
            if let Some(status) = visit(stage, stmt).steer(&mut cursor) {
                return status;
            }
        }
        WalkStatus::Completed
    }

    /// [`walk`](Self::walk) with a `visit` that may edit through `rewriter`,
    /// within the walk's contract (see the [module docs](super)). Fails if
    /// `visit` erases the statement it is visiting.
    fn walk_mut<'r, L: Dialect>(
        rewriter: &mut Rewriter<'r, L>,
        root: Statement,
        mut visit: impl FnMut(&mut Rewriter<'r, L>, Statement) -> WalkResult,
    ) -> Result<WalkStatus, WalkError> {
        let mut cursor = Self::start(rewriter.stage(), root);
        while let Some(stmt) = cursor.next(rewriter.stage())? {
            if let Some(status) = visit(rewriter, stmt).steer(&mut cursor) {
                return Ok(status);
            }
        }
        Ok(WalkStatus::Completed)
    }
}

/// What a walk does after visiting a statement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WalkResult {
    /// Go on to the next statement.
    #[default]
    Advance,
    /// Go on, without walking the statements nested in this one. See
    /// [`WalkCursor::skip`] for the orders where this has an effect.
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

/// A position in a walk. It holds only statement ids, so the IR can be
/// edited between steps.
pub trait WalkCursor {
    /// The next statement to visit, or `None` once the walk is done.
    ///
    /// The IR may be edited between calls within the walk's contract (see
    /// the [module docs](super)). Fails if the statement returned by the
    /// previous call has since been erased.
    fn next<L: Dialect>(&mut self, stage: &StageInfo<L>) -> Result<Option<Statement>, WalkError>;

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
    /// The statement being visited was erased before the walk moved on.
    VisitedErased(Statement),
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
    /// Mark the statement the cursor is at as skipped; nothing to mark
    /// before the first statement or past the last.
    pub(super) fn skip(&mut self) {
        if let Position::At(stmt) = *self {
            *self = Position::Skipped(stmt);
        }
    }
}
