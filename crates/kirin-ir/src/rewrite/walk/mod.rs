//! Traversal orders over the statement nesting tree, for rewrite drivers.
//!
//! The tree is rooted at a statement. A statement's children are the
//! statements in the bodies it owns, in a fixed order: its `Block`s, then its
//! `CFG`s (blocks in layout order), then its `DiGraph`s and `UnGraph`s
//! (members in member order). Within a block the terminator comes last.
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
//!   levels that own bodies. It sees statements inserted at the current
//!   level ahead of the cursor, and at levels still to come.
//! - [`ReverseBFS`] (each level right to left) and [`KeepOrderReverseBFS`]
//!   (each level left to right) build their order when the walk starts, in
//!   O(n). Statements inserted later are not visited; statements erased
//!   later are skipped.
//!
//! # Contract
//!
//! While a walk is visiting a statement, edits must not erase it or any of
//! its ancestors (erasing a statement erases everything nested in it). They
//! may add or erase anything else, including its siblings and the statements
//! nested in it.
//!
//! The cursor reports an erased visited statement as a [`WalkError`] on its
//! next step.

mod breadth_first;
mod depth_first;
mod order;
mod tree;

pub use breadth_first::{BFS, BFSCursor, BuiltOrderCursor, KeepOrderReverseBFS, ReverseBFS};
pub use depth_first::{PostOrder, PostOrderCursor, PreOrder, PreOrderCursor};
pub use order::{WalkCursor, WalkError, WalkOrder};
