//! The worklist fixpoint framework.
//!
//! [`StandardFixpointInterpreter`] wraps any [`Interp`](crate::Interp) and drives
//! it to a fixpoint over a worklist of work items: each item is walked on the
//! shared frame stack, and a [`WorkAnalysis`] builds its entry frame and handles
//! its completion. The wrapped interpreter stays the single source of
//! value/error/effect/semantics; a [`FixpointProfile`] adds only the work item,
//! frame and completion types.

mod delegates;
mod driver;
mod profile;
mod runner;
mod solver;
mod traits;

pub use driver::StandardFixpointInterpreter;
pub use profile::FixpointProfile;
pub use traits::WorkAnalysis;
