//! Restricted in-place mutation layer over finalized IR.
//!
//! First slice of the rewrite engine described in
//! `docs/design/rewrite-engine.md`, in two layers:
//!
//! - [`Rewriter`] is the mutation entry point, with its
//!   [`MutationEvent`]/[`RewriteError`] vocabulary. It checks each edit's own
//!   preconditions and keeps every derived mirror (e.g. uses, predecessors)
//!   in step, one edit at a time.
//! - [`run_pass`] is the boundary around a whole pass. It takes ownership of
//!   the stage, lends a `Rewriter` to the pass, and verifies the derived
//!   metadata before handing the stage back. Any failure, such as an error,
//!   a panic, or a stale mirror, produces a [`Quarantined`] stage instead,
//!   holding the mutated IR for diagnosis but not for further use.
//!
//! See the [`rewriter`] module docs for what a single edit does and does not
//! guarantee, and [`pass`] for the boundary's failure routes.

pub(crate) mod pass;
pub(crate) mod quarantine;
pub(crate) mod rewriter;

pub use pass::run_pass;
pub use quarantine::{QuarantineCause, Quarantined};
pub use rewriter::{MutationEvent, RewriteError, Rewriter};
