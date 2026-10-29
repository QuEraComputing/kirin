//! Strong (true) liveness for Kirin: *which SSA values are transitively
//! needed by a root?*
//!
//! This crate is the sparse backward demand analysis: one [`Demand`] bit per
//! SSA value in each reached callable context, computed by [`analyze_demand`]
//! on the shared sparse backward engine
//! ([`kirin_interpreter::SparseBackwardInterpreter`]).
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
//! let result = kirin_demand::analyze_demand(&pipeline, stage, callee, [Demand::Live])?;
//! assert!(result.is_demanded(some_value));
//! ```

mod context;
mod result;
mod value;

pub use context::{DemandCallCtx, OutputDemandContext};
pub use result::DemandResult;
pub use value::Demand;

use kirin_interpreter::{
    Callee, InterpDispatch, InterpreterError, SameStageLinker, SparseBackwardDriver,
    SparseBackwardInterpreter, StageQuery,
};
use kirin_ir::{CompileStage, Pipeline, StageMeta};

/// The sparse backward demand engine instantiated at the [`Demand`] lattice:
/// strong liveness.
pub type DemandAnalysis<
    'ir,
    S,
    E = InterpreterError,
    Lk = SameStageLinker,
    P = OutputDemandContext,
> = SparseBackwardInterpreter<'ir, S, Demand, E, Lk, P>;

/// Resolve `callee` and run strong liveness (sparse backward demand) from
/// its callable body. Supply one lattice value per result, in result order.
pub fn analyze_demand<'ir, S>(
    pipeline: &'ir Pipeline<S>,
    stage: CompileStage,
    callee: Callee,
    result_values: impl IntoIterator<Item = Demand>,
) -> Result<DemandResult, InterpreterError>
where
    S: StageMeta
        + StageQuery
        + InterpDispatch<
            SparseBackwardDriver<
                'ir,
                S,
                Demand,
                InterpreterError,
                SameStageLinker,
                OutputDemandContext,
            >,
        >,
{
    let (_, engine) = DemandAnalysis::<S>::new(pipeline).analyze(stage, callee, result_values)?;
    DemandResult::from_engine(&engine)
}
