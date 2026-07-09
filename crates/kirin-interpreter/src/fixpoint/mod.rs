//! The owner-summary fixpoint framework.
//!
//! [`StandardFixpointInterpreter`] wraps any [`Interp`](crate::Interp) and drives
//! it to an owner-summary fixpoint: one work item per summary owner, intra-owner
//! traversal on the shared frame stack, inter-owner convergence via a pluggable
//! [`SummaryDependencyIndex`]. The wrapped interpreter stays the single source of
//! value/error/effect/semantics; a [`FixpointProfile`] adds only the owner-summary
//! types (owner key, summary, frame, completion).

mod delegates;
mod deps;
mod driver;
mod profile;
mod runner;
mod solver;
mod traits;

#[cfg(test)]
mod tests;

pub use deps::{
    BackwardSummaryDeps, ForwardSummaryDeps, OwnerSummaryDeps, SummaryDependencies,
    SummaryDependency, SummaryDependencyIndex,
};
pub use driver::{SimpleFixpointInterpreter, StandardFixpointInterpreter};
pub use profile::FixpointProfile;
pub use traits::{FixpointPhase, OwnerSemantics, Summary, SummaryEffect, WorkItem};
