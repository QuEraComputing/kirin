//! Restricted in-place mutation layer over finalized IR.
//!
//! First slice of the rewrite engine described in
//! `docs/design/rewrite-engine.md`: the [`Rewriter`] mutation entry point
//! with its [`MutationEvent`]/[`RewriteError`] vocabulary.
//!
//! Per-action preconditions are checked, but whole-stage validity is a
//! *pass-boundary* property that is not implemented yet. See the
//! [`rewriter`] module docs for exactly what is and is not guaranteed.

pub(crate) mod rewriter;

pub use rewriter::{MutationEvent, RewriteError, Rewriter};
