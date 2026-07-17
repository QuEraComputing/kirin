//! The **concrete** implementation of the shared [`frame`](crate::core::frame)
//! protocol.
//!
//! Two independent axes organize these frames:
//!
//! - **Body representation** — the closed [`Body`](crate::Body) vocabulary
//!   (`CFG` / `Block` / `DiGraph` / `UnGraph`), an intentional IR design
//!   decision. Each representation the framework can walk has one
//!   representation frame implementing *traversal mechanics only*:
//!   [`CFGFrame`] (multi-block, follows jumps), [`BlockFrame`] (one linear
//!   block), and [`DiGraphFrame`] (dependency-ordered DAG walk). `UnGraph`
//!   has **no** default walker — an undirected graph has no inherent
//!   execution order, so traversal is a dialect/compiler-supplied policy
//!   ([`FrameBuild::from_ungraph_entry`]).
//!
//! - **Entry context** — *why* a body is being walked: as a callable function
//!   body (entered through [`CallFrame`], the call boundary that owns the
//!   callee activation and return bookkeeping), as a nested structured
//!   operation body (entered through a dialect frame such as `kirin-scf`'s
//!   `ScfIfFrame`/`ScfForFrame`, pushed with
//!   [`SparseForwardEffect::Push`]), or as an analysis owner (the abstract
//!   engines' concern, not this module's).
//!
//! Representation frames never know their entry context: the same
//! [`BlockFrame`] walks a callable Block body and an `scf.if` arm; the parent
//! frame ([`CallFrame`] or the dialect frame) interprets the walker's
//! [`Completion`] and owns activation lifetime. Roles compose instead of
//! multiplying frame types:
//!
//! ```text
//! linear function  = CallFrame   → BlockFrame
//! CFG function     = CallFrame   → CFGFrame
//! graph function   = CallFrame   → DiGraphFrame
//! nested scf block = ScfIfFrame  → BlockFrame
//! ```
//!
//! These are the default total frames for
//! [`ConcreteInterpreter`](crate::ConcreteInterpreter) (bundled as
//! [`StandardFrame`]). Structured-control dialects do not get a framework
//! "scope": they push a frame **they own** through
//! [`SparseForwardEffect::Push`] (that frame may build a [`BlockFrame`] to
//! walk a chosen body — a reusable building block, not framework-owned
//! structured semantics). A language that combines such a dialect defines its
//! own total frame enum embedding these frames via [`FrameBuild`] plus its
//! dialect frames. The forward abstract analogue lives in
//! [`sparse_forward::frames`](crate::engines::sparse_forward::frames).
//!
//! [`SparseForwardEffect::Push`]: crate::SparseForwardEffect::Push

mod block_cursor;
mod block_frame;
mod call_frame;
mod cfg_frame;
mod digraph_frame;
mod protocol;
mod standard_frame;

pub use block_frame::BlockFrame;
pub use call_frame::CallFrame;
pub use cfg_frame::CFGFrame;
pub use digraph_frame::DiGraphFrame;
pub use protocol::{Completion, FrameBuild, UnGraphEntry};
pub use standard_frame::StandardFrame;
