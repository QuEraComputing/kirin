use std::fmt;

use crate::{Dialect, StageInfo, Statement};

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
    Done,
}
