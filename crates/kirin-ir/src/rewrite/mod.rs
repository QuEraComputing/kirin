//! Safe mutation layer over finalized IR.
//!
//! First slice of the rewrite engine described in
//! `docs/design/rewrite-engine.md`: the [`Rewriter`] mutation entry point
//! with its [`MutationEvent`]/[`RewriteError`] vocabulary.

mod rewriter;

pub use rewriter::{MutationEvent, RewriteError, Rewriter};
