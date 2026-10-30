//! Traversal orders over the statement nesting tree, for rewrite drivers.
//!
//! The tree is rooted at a statement. A statement's children are the
//! statements in the bodies it owns, in a fixed order: its `Block`s, then its
//! `CFG`s (blocks in layout order), then its `DiGraph`s and `UnGraph`s
//! (members in member order). Within a block the terminator comes last.
//!
//! Only a block's statement order is program order. A `CFG`'s layout order
//! and a `DiGraph`'s or `UnGraph`'s member order carry no meaning (control
//! flow is in the terminators, connectivity in the graph's edges), so the
//! walks use them only as a deterministic order. Member order is statement
//! id order. Rewrites whose result depends on visit order therefore depend
//! on these orders across blocks and graph members.
//!
//! A walk is a [`WalkCursor`] stepped one statement at a time, so the IR can
//! be edited at each statement before the walk moves on:
//!
//! - [`PreOrder`] and [`PostOrder`] follow the IR's own links in O(1) extra
//!   space, reading them at every step, so they visit statements inserted
//!   ahead of the cursor but not ones inserted where it has already been.
//!   [`PreOrder`] reads a statement's children after visiting it, so it
//!   walks children added there. [`PostOrder`] visits a statement after its
//!   children and never goes back into it, so children added while visiting
//!   it are not walked until the next walk.
//! - [`BFS`] follows the links too, keeping the statements of two adjacent
//!   levels that own bodies. It discovers children through queued parents
//!   as their bodies are walked. Inserts ahead of the cursor in those
//!   bodies are visited; edits to bodies already passed, or to statements
//!   not queued as parents, may be missed together with their descendants,
//!   even at depths the walk has not reached. See [`BFSCursor`] for details.
//! - [`ReverseBFS`] (each level right to left) and [`KeepOrderReverseBFS`]
//!   (each level left to right) build their order when the walk starts, in
//!   O(n). Statements inserted later are not visited; statements erased
//!   later are skipped.
//!
//! # Steering a walk
//!
//! [`WalkOrder::walk`] and [`WalkOrder::walk_mut`] call a visit function at
//! each statement and act on the [`WalkResult`] it returns:
//!
//! - [`WalkResult::Advance`] goes on to the next statement.
//! - [`WalkResult::Skip`] goes on without walking the statements nested in
//!   this one, through [`WalkCursor::skip`]. [`PreOrder`] steps past them and
//!   [`BFS`] never queues them. The other orders have already visited them,
//!   so there it is the same as `Advance`.
//! - [`WalkResult::Interrupt`] ends the walk, which reports
//!   [`WalkStatus::Interrupted`]. Edits already made stay.
//!
//! A cursor driven by hand gets the same control: calling
//! [`WalkCursor::skip`] skips, and no longer calling `next` interrupts.
//!
//! # Contract
//!
//! While a walk is visiting a statement, edits may add or erase statements
//! anywhere, including its siblings and the statements nested in it.
//! Erasing a statement erases everything nested in it. The one limit is on
//! erasing the visited statement or one of its ancestors, which depends on
//! the order:
//!
//! - [`PreOrder`] and [`PostOrder`]: neither allowed. They find the next
//!   statement through the visited one's links.
//! - [`BFS`]: erasing an ancestor allowed, which erases the visited
//!   statement and its siblings with it, but not the visited statement
//!   alone. Its siblings would still be live but unreachable.
//! - [`ReverseBFS`] and [`KeepOrderReverseBFS`]: both allowed. Their order
//!   is built at the start, and erased statements are skipped.
//!
//! A moved statement is treated as erased from its old place and inserted at
//! its new one. Moving the visited statement itself is dangerous in
//! [`PreOrder`], [`PostOrder`] and [`BFS`]: they find the next statement
//! from the visited one, so the walk would carry on from its new place,
//! skipping or repeating part of the walk. These orders do not allow:
//!
//! - merging its block into another block
//!   ([`Rewriter::merge_blocks`](crate::Rewriter::merge_blocks) with its
//!   block as `succ`), reported as [`WalkError::VisitedMoved`];
//! - reordering the bodies of one of its ancestors
//!   ([`Rewriter::replace_statement`](crate::Rewriter::replace_statement)),
//!   which is not checked.
//!
//! Splitting its block with
//! [`Rewriter::split_block`](crate::Rewriter::split_block) keeps the order
//! and is allowed. [`ReverseBFS`] and [`KeepOrderReverseBFS`] allow every
//! move: they visit each statement once, where it was at the start.
//!
//! An allowed edit does not guarantee that newly nested statements will be
//! visited in this walk. Discovery follows each order's rules above; nesting
//! depth alone does not determine whether an insertion is ahead of the cursor.
//!
//! The cursor reports an erase or merge of the visited statement that its
//! order does not allow as a [`WalkError`] on its next step.

mod breadth_first;
mod depth_first;
mod order;
mod tree;

pub use breadth_first::{BFS, BFSCursor, BuiltOrderCursor, KeepOrderReverseBFS, ReverseBFS};
pub use depth_first::{PostOrder, PostOrderCursor, PreOrder, PreOrderCursor};
pub use order::{WalkCursor, WalkError, WalkOrder, WalkResult, WalkStatus};
