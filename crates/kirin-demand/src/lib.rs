//! Strong (true) liveness for Kirin: *which SSA values are transitively
//! needed by a root?*
//!
//! This crate is the sparse backward demand analysis: one [`Demand`] bit per
//! SSA value in each reached callable context, computed by [`DemandAnalysis`],
//! the shared sparse backward engine
//! ([`kirin_interpreter::SparseBackwardInterpreter`]) at the [`Demand`]
//! lattice.
//! Roots are the requested outputs of the analyzed callable, control roots
//! (branch conditions, loop bounds), and impure ordinary statements. Demand
//! crosses supported calls: a demanded call result requests the matching
//! callee output, and only the callee parameters that output needs demand
//! their call arguments. An ignored call demands nothing — this analysis
//! tracks demandedness, not side effects, so its results alone do not license
//! deleting effectful calls. A dead pure statement demands nothing.
//! Transfer comes from each dialect's `Interpretable<I, StrongDemand>` rule —
//! there is no structural fallback.
//!
//! ```ignore
//! use kirin_interpreter::AnalysisResults;
//!
//! let analyzed = DemandAnalysis::<Stage>::new(&pipeline).analyze(stage, callee, [Demand::Live])?;
//! assert!(analyzed.is_demanded(analyzed.root(), some_value));
//! ```

mod context;
mod value;

pub use context::{DemandCallCtx, OutputDemandContext};
pub use value::Demand;

use kirin_interpreter::{InterpreterError, SameStageLinker, SparseBackwardInterpreter};

/// The sparse backward demand engine instantiated at the [`Demand`] lattice:
/// strong liveness.
pub type DemandAnalysis<
    'ir,
    S,
    E = InterpreterError,
    Lk = SameStageLinker,
    P = OutputDemandContext,
> = SparseBackwardInterpreter<'ir, S, Demand, E, Lk, P>;
