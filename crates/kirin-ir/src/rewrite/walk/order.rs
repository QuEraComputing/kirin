use std::fmt;

use crate::arena::GetInfo;
use crate::{
    Block, BlockParent, CFG, Dialect, MutationEvent, Rewriter, StageInfo, Statement,
    StatementParent,
};

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

    /// Whether an edit may erase or move the statement being visited (see
    /// the [contract](super#contract)). Only the reverse BFS orders allow it:
    /// they build their order at the start, while the others find the next
    /// statement through the visited one.
    const ALLOWS_DISPLACING_VISITED: bool;

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
    /// `visit` erases the statement it is visiting where the order does not
    /// allow it, or moves it where the order does not allow it (see
    /// [`check_not_moved_since`](Self::check_not_moved_since)).
    fn walk_mut<'r, L: Dialect>(
        rewriter: &mut Rewriter<'r, L>,
        root: Statement,
        mut visit: impl FnMut(&mut Rewriter<'r, L>, Statement) -> WalkResult,
    ) -> Result<WalkStatus, WalkError> {
        let mut cursor = Self::start(rewriter.stage(), root);
        while let Some(stmt) = cursor.next(rewriter.stage())? {
            let seen = rewriter.events().len();
            let place = VisitedPlace::of(rewriter.stage(), stmt);
            let result = visit(rewriter, stmt);
            Self::check_not_moved_since(rewriter, stmt, place, seen)?;
            if let Some(status) = result.steer(&mut cursor) {
                return Ok(status);
            }
        }
        Ok(WalkStatus::Completed)
    }

    /// Fail with [`WalkError::VisitedMoved`] if an event recorded since the
    /// first `seen` events of `rewriter` moved `visited`, in an order that
    /// does not allow it
    /// ([`ALLOWS_DISPLACING_VISITED`](Self::ALLOWS_DISPLACING_VISITED)):
    /// a move of `visited` itself, or a splice of the block or CFG it was in
    /// at `place`. Panics if `rewriter` has recorded fewer than `seen` events.
    ///
    /// [`walk_mut`](Self::walk_mut) calls this after each visit. A loop driving
    /// a cursor by hand calls it too: note `rewriter.events().len()` and
    /// [`VisitedPlace::of`] before editing at a statement, and check before
    /// the next [`WalkCursor::next`].
    fn check_not_moved_since<L: Dialect>(
        rewriter: &Rewriter<'_, L>,
        visited: Statement,
        place: VisitedPlace,
        seen: usize,
    ) -> Result<(), WalkError> {
        if Self::ALLOWS_DISPLACING_VISITED {
            return Ok(());
        }
        let moved = rewriter.events()[seen..].iter().any(|event| match *event {
            MutationEvent::MovedStatement { stmt } => stmt == visited,
            MutationEvent::SplicedBlock { source, .. } => place.block == Some(source),
            MutationEvent::SplicedCfg { source, .. } => place.cfg == Some(source),
            _ => false,
        });
        if moved {
            Err(WalkError::VisitedMoved(visited))
        } else {
            Ok(())
        }
    }
}

/// Where a visited statement was when its visit started: its block, and that
/// block's CFG. [`WalkOrder::check_not_moved_since`] compares splices against
/// it, since a splice changes both: afterwards, a statement spliced into a
/// block, or a block spliced into a CFG, cannot be told from one that was
/// there already.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisitedPlace {
    block: Option<Block>,
    cfg: Option<CFG>,
}

impl VisitedPlace {
    /// Where `stmt` is now in `stage`.
    pub fn of<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Self {
        let block = match stmt.get_info(stage).and_then(|info| info.parent) {
            Some(StatementParent::Block(block)) => Some(block),
            _ => None,
        };
        let cfg = match block.and_then(|block| block.get_info(stage)?.parent()) {
            Some(BlockParent::CFG(cfg)) => Some(cfg),
            _ => None,
        };
        Self { block, cfg }
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

/// A position in a walk. It holds only ids, so the IR can be edited between
/// steps.
pub trait WalkCursor {
    /// The next statement to visit, or `None` once the walk is done.
    ///
    /// The IR may be edited between calls within the walk's contract (see
    /// the [module docs](super)). Fails if the statement returned by the
    /// previous call has since been erased where the order does not allow
    /// it, or moved by merging its block into another.
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
    /// The statement being visited was erased before the walk proceeded, in a
    /// way its order does not allow (see the [module docs](super)).
    VisitedErased(Statement),
    /// The statement being visited was moved before the walk proceeded, in an
    /// order a move breaks: by merging its block into another block
    /// ([`Rewriter::merge_blocks`](crate::Rewriter::merge_blocks) with its
    /// block as `succ`), or with
    /// [`Rewriter::move_before`](crate::Rewriter::move_before) /
    /// [`Rewriter::move_after`](crate::Rewriter::move_after), or by splicing
    /// its block with
    /// [`Rewriter::splice_block_before`](crate::Rewriter::splice_block_before) /
    /// [`Rewriter::splice_block_after`](crate::Rewriter::splice_block_after),
    /// or by splicing its CFG with
    /// [`Rewriter::splice_cfg_after`](crate::Rewriter::splice_cfg_after) (see
    /// [`WalkOrder::check_not_moved_since`]).
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
                    "statement {stmt:?} was moved while the walk was visiting it"
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
