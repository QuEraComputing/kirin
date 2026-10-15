//! `CFG` block-list surgery for the [`Rewriter`](super::Rewriter).
//!
//! These methods add, erase, split, and merge blocks of a [`CFG`](crate::CFG),
//! retarget control-flow edges, and insert or erase block arguments together
//! with the incoming edges that forward them. Like statement surgery they
//! maintain every derived mirror on each successful edit —
//! [`SSAInfo::uses`](crate::SSAInfo),
//! [`BlockInfo::predecessors`](crate::BlockInfo), the block-body summary
//! (`BlockInfo::statements`/`terminator`), and the CFG chain summary
//! ([`CFGInfo::blocks`](crate::node::CFGInfo)) — and check executable
//! preconditions before writing.
//!
//! They never leave a dangling reference. Removing a block (erasing it, or
//! merging it into another) is rejected while some other block still
//! branches to it ([`RewriteError::BlockHasPredecessors`](super::RewriteError::BlockHasPredecessors))
//! or a value it defines is used outside it; retarget those edges and replace
//! those uses first. Other CFG well-formedness (terminators present, edge
//! arguments matching block arguments) may be broken between calls and is
//! left to verification.
//!
//! Erasure tombstones eagerly: a block's arguments, statements, results, and
//! nested statement-owned blocks and `CFG`s are marked deleted in one walk.
//! Liveness therefore stays a single per-item flag; nothing is "dead because
//! its parent is". (`DiGraph`/`UnGraph` bodies are not tombstoned yet.)
//!
//! Every block-level method requires its blocks to be members of a `CFG`
//! block list; statement-owned single-block bodies are rejected with
//! [`RewriteError::NotInCfgBody`](super::RewriteError::NotInCfgBody).

mod blocks;
mod edges;
mod helpers;
mod merge;
mod split;
