//! Breadth-first orders.
//!
//! [`BFS`] follows the IR's links, keeping only the statements of the
//! current and next level that own bodies. [`ReverseBFS`] and
//! [`KeepOrderReverseBFS`] need the deepest level first, so they build their
//! order when the walk starts, in O(n) time and space.

use super::depth_first::check_live;
use super::order::Position;
use super::tree::{append_children, first_child, next_sibling, owns_bodies};
use super::{WalkCursor, WalkError, WalkOrder};
use crate::{Dialect, GetInfo, StageInfo, Statement};

/// Breadth-first by nesting depth: the root, then every statement nested
/// directly in it, then every statement nested two levels down, and so on.
/// Each level is in program order.
/// See [`BFSCursor`] for how edits affect discovery during a walk.
#[derive(Clone, Copy, Debug, Default)]
pub struct BFS;

/// [`BFS`]'s levels from the deepest up to the root, each level right to
/// left (last statement first), so within a block uses come before their
/// definitions.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReverseBFS;

/// [`BFS`]'s levels from the deepest up to the root, each level left to
/// right (first statement first), so within a block definitions come before
/// their uses.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeepOrderReverseBFS;

/// A [`BFS`] walk in progress.
///
/// When the cursor moves on from a visited statement, it queues that statement
/// as a parent for the next level only if it owns bodies and was not skipped.
/// At the next level, the cursor walks each queued parent's bodies in turn,
/// reading their current links. Inserts ahead of the cursor in those bodies
/// are visited.
///
/// The cursor does not revisit bodies it has passed or reconsider statements
/// it did not queue as parents. For example, if a leaf gains a body after the
/// cursor has moved on from it, that body and its descendants are not visited.
/// Likewise, a subtree inserted behind the cursor is not discovered, even if
/// its descendants lie at depths the walk has not reached. Such edits are
/// allowed by the walk contract; visiting them requires another walk.
#[derive(Clone, Debug)]
pub struct BFSCursor {
    root: Statement,
    position: Position,
    /// The previous level's statements that own bodies; the current level is
    /// their children.
    parents: Vec<Statement>,
    /// The index in `parents` of the statement whose bodies are being read.
    parent_index: usize,
    /// The current level's statements that own bodies, collected as they are
    /// visited: the next level's parents.
    next_parents: Vec<Statement>,
}

/// A [`ReverseBFS`] or [`KeepOrderReverseBFS`] walk in progress: the order
/// built at the start, and how far through it the walk is.
///
/// Statements inserted after the start are not in the order, so they are
/// not visited. Statements erased after the start are skipped.
#[derive(Clone, Debug)]
pub struct BuiltOrderCursor {
    ordered_nodes: Vec<Statement>,
    /// The index of the next statement to consider.
    next: usize,
    /// The statement the previous call returned.
    visited: Option<Statement>,
}

impl WalkOrder for BFS {
    type Cursor = BFSCursor;

    fn start<L: Dialect>(_stage: &StageInfo<L>, root: Statement) -> Self::Cursor {
        BFSCursor {
            root,
            position: Position::Start,
            parents: Vec::new(),
            parent_index: 0,
            next_parents: Vec::new(),
        }
    }
}

impl WalkCursor for BFSCursor {
    fn next<L: Dialect>(&mut self, stage: &StageInfo<L>) -> Result<Option<Statement>, WalkError> {
        let next = match self.position {
            Position::Start => root_if_live(stage, self.root),
            Position::At(visited) | Position::Skipped(visited) => {
                check_live(stage, visited)?;
                // Keep it as a parent for the next level, whose statements
                // are its children. Statements without bodies have none,
                // and a skipped statement's are not walked.
                let skipped = matches!(self.position, Position::Skipped(_));
                if !skipped && owns_bodies(stage, visited) {
                    self.next_parents.push(visited);
                }
                // The root is the only statement of its level; every other
                // statement is a child of `parents[parent_index]`.
                let sibling = if visited == self.root {
                    None
                } else {
                    next_sibling(stage, visited)
                };
                // No next sibling: the current parent's children are done
                // processing, so move to the next parent's.
                sibling.or_else(|| {
                    if visited != self.root {
                        self.parent_index += 1;
                    }
                    self.first_child_of_next_parent(stage)
                })
            }
            Position::Done => None,
        };
        self.position = next.map_or(Position::Done, Position::At);
        Ok(next)
    }

    fn skip(&mut self) {
        self.position.skip();
    }
}

impl BFSCursor {
    /// The first child of `parents[parent_index]` or a later parent, moving
    /// on to the next level when this one runs out. `None` once a level has
    /// no statements that own bodies.
    fn first_child_of_next_parent<L: Dialect>(
        &mut self,
        stage: &StageInfo<L>,
    ) -> Option<Statement> {
        loop {
            while let Some(&parent) = self.parents.get(self.parent_index) {
                // Skip parents with no children: their bodies are empty, or
                // they were erased.
                if let Some(child) = first_child(stage, parent) {
                    return Some(child);
                }
                self.parent_index += 1;
            }
            if self.next_parents.is_empty() {
                return None;
            }
            // The level is done: its collected statements (`next_parents`)
            // become the parents at the next level, and the finished
            // parents' vector is emptied (keeping its capacity) to collect
            // the level after.
            std::mem::swap(&mut self.parents, &mut self.next_parents);
            self.next_parents.clear();
            // Start reading from the new level's first parent.
            self.parent_index = 0;
        }
    }
}

/// `root` if it is live.
fn root_if_live<L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Option<Statement> {
    root.get_info(stage).map(|_| root)
}

impl WalkOrder for ReverseBFS {
    type Cursor = BuiltOrderCursor;

    fn start<L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Self::Cursor {
        let mut ordered_nodes = breadth_first(stage, root, |_| {});
        ordered_nodes.reverse();
        BuiltOrderCursor::new(ordered_nodes)
    }
}

impl WalkOrder for KeepOrderReverseBFS {
    type Cursor = BuiltOrderCursor;

    fn start<L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Self::Cursor {
        let mut level_starts = Vec::new();
        let mut ordered_nodes = breadth_first(stage, root, |start| level_starts.push(start));

        // Reversing the whole order puts the deepest level first but leaves
        // each level backwards; reversing each level's slice restores it.
        let len = ordered_nodes.len();
        ordered_nodes.reverse();
        for (index, &start) in level_starts.iter().enumerate() {
            let end = level_starts.get(index + 1).copied().unwrap_or(len);
            ordered_nodes[len - end..len - start].reverse();
        }
        BuiltOrderCursor::new(ordered_nodes)
    }
}

impl BuiltOrderCursor {
    fn new(ordered_nodes: Vec<Statement>) -> Self {
        Self {
            ordered_nodes,
            next: 0,
            visited: None,
        }
    }
}

impl WalkCursor for BuiltOrderCursor {
    fn next<L: Dialect>(&mut self, stage: &StageInfo<L>) -> Result<Option<Statement>, WalkError> {
        if let Some(visited) = self.visited.take() {
            check_live(stage, visited)?;
        }
        // Return the next statement still live, skipping any erased since the
        // order was built.
        while let Some(&stmt) = self.ordered_nodes.get(self.next) {
            self.next += 1;
            if stmt.get_info(stage).is_some() {
                self.visited = Some(stmt);
                return Ok(Some(stmt));
            }
        }
        Ok(None)
    }

    /// Nothing to skip: the order visits a statement after the statements
    /// nested in it.
    fn skip(&mut self) {}
}

/// The statements under `root` in breadth-first order, for [`ReverseBFS`]
/// and [`KeepOrderReverseBFS`], calling
/// `level_start` with the index each nesting level starts at.
///
/// The order doubles as the queue: the statement at `index` has its children
/// appended to the end, so there is no separate frontier.
fn breadth_first<L: Dialect>(
    stage: &StageInfo<L>,
    root: Statement,
    mut level_start: impl FnMut(usize),
) -> Vec<Statement> {
    if root.get_info(stage).is_none() {
        return Vec::new();
    }
    let mut ordered_nodes = vec![root];
    let mut level_end = 0;
    let mut index = 0;
    while index < ordered_nodes.len() {
        if index == level_end {
            level_start(index);
            level_end = ordered_nodes.len();
        }
        append_children(stage, ordered_nodes[index], &mut ordered_nodes);
        index += 1;
    }
    ordered_nodes
}
