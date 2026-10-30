//! Depth-first orders, walked through the IR's own links in O(1) extra space.

use super::order::Position;
use super::tree::{first_child, next_sibling, parent};
use super::{WalkCursor, WalkError, WalkOrder};
use crate::{Block, Dialect, GetInfo, StageInfo, Statement, StatementParent};

/// Depth-first, each statement before the statements nested in it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PreOrder;

/// Depth-first, each statement after the statements nested in it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PostOrder;

/// A [`PreOrder`] walk in progress.
#[derive(Clone, Copy, Debug)]
pub struct PreOrderCursor {
    root: Statement,
    position: Position,
    /// The block the visited statement was in when visited.
    visited_block: Option<Block>,
}

/// A [`PostOrder`] walk in progress.
#[derive(Clone, Copy, Debug)]
pub struct PostOrderCursor {
    root: Statement,
    position: Position,
    /// The block the visited statement was in when visited.
    visited_block: Option<Block>,
}

impl WalkOrder for PreOrder {
    type Cursor = PreOrderCursor;

    fn start<L: Dialect>(_stage: &StageInfo<L>, root: Statement) -> Self::Cursor {
        PreOrderCursor {
            root,
            position: Position::Start,
            visited_block: None,
        }
    }
}

impl WalkOrder for PostOrder {
    type Cursor = PostOrderCursor;

    fn start<L: Dialect>(_stage: &StageInfo<L>, root: Statement) -> Self::Cursor {
        PostOrderCursor {
            root,
            position: Position::Start,
            visited_block: None,
        }
    }
}

impl WalkCursor for PreOrderCursor {
    fn next<L: Dialect>(&mut self, stage: &StageInfo<L>) -> Result<Option<Statement>, WalkError> {
        let next = match self.position {
            Position::Start => live(stage, self.root),
            Position::At(visited) => {
                check_in_place(stage, visited, self.visited_block)?;
                // The children are read now, so edits made while visiting
                // `visited` are walked.
                first_child(stage, visited).or_else(|| after_subtree(stage, self.root, visited))
            }
            Position::Skipped(visited) => {
                check_in_place(stage, visited, self.visited_block)?;
                after_subtree(stage, self.root, visited)
            }
            Position::Done => None,
        };
        self.position = next.map_or(Position::Done, Position::At);
        self.visited_block = visited_block(stage, self.root, next);
        Ok(next)
    }

    fn skip(&mut self) {
        self.position.skip();
    }
}

impl WalkCursor for PostOrderCursor {
    fn next<L: Dialect>(&mut self, stage: &StageInfo<L>) -> Result<Option<Statement>, WalkError> {
        let next = match self.position {
            Position::Start => live(stage, self.root).map(|root| deepest_first(stage, root)),
            // Never `Skipped`: `skip` does nothing in this order.
            Position::At(visited) | Position::Skipped(visited) => {
                check_in_place(stage, visited, self.visited_block)?;
                if visited == self.root {
                    None
                } else if let Some(sibling) = next_sibling(stage, visited) {
                    Some(deepest_first(stage, sibling))
                } else {
                    parent(stage, visited)
                }
            }
            Position::Done => None,
        };
        self.position = next.map_or(Position::Done, Position::At);
        self.visited_block = visited_block(stage, self.root, next);
        Ok(next)
    }

    /// Nothing to skip: the children were visited before the statement.
    fn skip(&mut self) {}
}

/// `stmt` if it is live.
pub(super) fn live<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Option<Statement> {
    stmt.get_info(stage).map(|_| stmt)
}

/// Fail if the statement just visited was erased.
fn check_live<L: Dialect>(stage: &StageInfo<L>, visited: Statement) -> Result<(), WalkError> {
    live(stage, visited)
        .map(|_| ())
        .ok_or(WalkError::VisitedErased(visited))
}

/// Fail if the statement just visited was erased, or moved by merging
/// `block`, the block it was in when visited, into another block.
fn check_in_place<L: Dialect>(
    stage: &StageInfo<L>,
    visited: Statement,
    block: Option<Block>,
) -> Result<(), WalkError> {
    check_live(stage, visited)?;
    check_not_moved(stage, visited, block)
}

/// Fail if the live statement just visited left `block`, the block it was
/// in when visited, because `block` was merged into another block. Only a
/// merge erases a block and keeps its statements, and splitting a block
/// keeps the original live, so a split passes.
pub(super) fn check_not_moved<L: Dialect>(
    stage: &StageInfo<L>,
    visited: Statement,
    block: Option<Block>,
) -> Result<(), WalkError> {
    match block {
        Some(block) if block.get_info(stage).is_none() => Err(WalkError::VisitedMoved(visited)),
        _ => Ok(()),
    }
}

/// The block `next` is in, for [`check_not_moved`] at the next step. `None`
/// for the root, whose place outside its subtree the walk never reads, so
/// moving it is harmless.
pub(super) fn visited_block<L: Dialect>(
    stage: &StageInfo<L>,
    root: Statement,
    next: Option<Statement>,
) -> Option<Block> {
    let stmt = next.filter(|&stmt| stmt != root)?;
    match stmt.get_info(stage)?.parent? {
        StatementParent::Block(block) => Some(block),
        StatementParent::DiGraph(_) | StatementParent::UnGraph(_) => None,
    }
}

/// The first statement after `stmt`'s subtree within `root`'s: the next
/// sibling of `stmt` or of its nearest ancestor that has one, stopping at
/// `root`.
fn after_subtree<L: Dialect>(
    stage: &StageInfo<L>,
    root: Statement,
    stmt: Statement,
) -> Option<Statement> {
    let mut current = stmt;
    while current != root {
        if let Some(sibling) = next_sibling(stage, current) {
            return Some(sibling);
        }
        current = parent(stage, current)?;
    }
    None
}

/// The first statement [`PostOrder`] visits under `stmt`: its first child's
/// first child, and so on.
fn deepest_first<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Statement {
    let mut current = stmt;
    while let Some(child) = first_child(stage, current) {
        current = child;
    }
    current
}
