//! SSA-value liveness for Kirin.
//!
//! **Strong (true) liveness** — *which SSA values are transitively needed by a
//! root?* — is the sparse backward demand analysis: one [`Live`] bit per SSA
//! value, computed by [`analyze_demand`] on the shared sparse backward engine
//! ([`SparseBackwardInterpreter`](kirin_interpreter::SparseBackwardInterpreter)).
//! Roots are return/terminator operands and impure statements (calls demand
//! their arguments even when their results are unused); a dead pure statement
//! demands nothing, so DCE can consume [`DemandResult::is_demanded`] directly.
//! Transfer comes from each dialect's `Interpretable<I, SparseBackward>` rule —
//! there is no structural fallback.
//!
//! Classic per-program-point liveness (a [`LiveSet`] per point — the
//! regalloc-grade fact) is the *dense* backward analysis; strong per-point
//! sets are the intersection of the dense sets with this demand set.
//!
//! ```ignore
//! let result = kirin_liveness::analyze_demand(&pipeline, stage, region)?;
//! assert!(result.is_demanded(some_value));
//! ```

mod live;
mod result;

pub use live::{Live, LiveSet};
pub use result::{DemandResult, DenseLivenessResult};

use kirin_interpreter::{
    DenseBackward, DenseBackwardInterpreter, DenseBackwardTransfer, InterpDispatch,
    InterpreterError, SparseBackward, SparseBackwardDriver, SparseBackwardInterpreter, StageQuery,
    StandardDenseBackwardFrame,
};
use kirin_ir::{CompileStage, Pipeline, Region, StageMeta};

/// The sparse backward demand engine instantiated at the [`Live`] lattice:
/// strong liveness.
pub type Demand<'ir, S, E = InterpreterError> = SparseBackwardInterpreter<'ir, S, Live, E>;

/// The dense backward engine instantiated at [`LiveSet`] point states:
/// classic per-program-point liveness. A language with structured dialects
/// supplies its own total frame `F` embedding the dialect's dense frames.
pub type DenseLiveness<'ir, S, E = InterpreterError, F = StandardDenseBackwardFrame<LiveSet, E>> =
    DenseBackwardInterpreter<'ir, S, LiveSet, E, F>;

/// Run strong liveness (sparse backward demand) over `region` in `stage`.
pub fn analyze_demand<'ir, S>(
    pipeline: &'ir Pipeline<S>,
    stage: CompileStage,
    region: Region,
) -> Result<DemandResult, InterpreterError>
where
    S: StageMeta
        + StageQuery
        + InterpDispatch<SparseBackwardDriver<'ir, S, Live, InterpreterError>, SparseBackward>,
{
    let mut engine = Demand::<S>::new(pipeline);
    engine.analyze(stage, region)?;
    Ok(DemandResult::from_engine(&engine, stage, region))
}

/// Run classic per-point liveness (dense backward) over `region` in `stage`,
/// with the standard (structured-control-free) frames. Languages with scf
/// compose [`DenseLiveness`] with their own frame type and build the result
/// via [`DenseLivenessResult::from_engine`].
pub fn analyze_dense<'ir, S>(
    pipeline: &'ir Pipeline<S>,
    stage: CompileStage,
    region: Region,
) -> Result<DenseLivenessResult, InterpreterError>
where
    S: StageMeta
        + StageQuery
        + InterpDispatch<
            DenseBackwardTransfer<
                'ir,
                S,
                LiveSet,
                InterpreterError,
                StandardDenseBackwardFrame<LiveSet, InterpreterError>,
            >,
            DenseBackward,
        >,
{
    let mut engine = DenseLiveness::<S>::new(pipeline);
    engine.analyze(stage, region)?;
    DenseLivenessResult::from_engine(&mut engine, stage, region)
}
