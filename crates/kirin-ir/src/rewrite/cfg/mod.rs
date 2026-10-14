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
//! They do **not** keep the CFG well-formed. Erasing a block that still has
//! predecessors, or merging a block that other blocks still branch to, is
//! accepted; the remaining edges then name a tombstoned block and
//! [`verify_derived`](crate::verify_derived) reports them as
//! [`Finding::DanglingSuccessor`](crate::Finding). Values, by contrast, are
//! never left dangling: erasing a block whose values are used outside it is
//! rejected.
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
