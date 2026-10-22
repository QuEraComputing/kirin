//! SSA-value liveness for Kirin.
//!
//! **Strong (true) liveness** — *which SSA values are transitively needed by a
//! root?* — is the sparse backward demand analysis: one [`Demand`] bit per SSA
//! value in each reached callable context, computed by [`analyze_demand`] on
//! the shared sparse backward engine
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
//! Classic per-program-point liveness (a [`LiveSet`] per point — the
//! regalloc-grade fact) is the *dense* backward analysis; strong per-point
//! sets are the intersection of the dense sets with this demand set.
//!
//! ```ignore
//! let result = kirin_liveness::analyze_demand(&pipeline, stage, callee, [Demand::Live])?;
//! assert!(result.is_demanded(some_value));
//! ```

mod context;
mod live;
mod result;

pub use context::{DemandCallCtx, OutputDemandContext};
pub use live::{Demand, LiveSet};
pub use result::{DemandResult, DenseLivenessResult};

use kirin_interpreter::{
    Callee, DenseBackwardCompletion, DenseBackwardDriver, DenseBackwardInterpreter,
    DenseBackwardTransfer, DenseBlockFrame, Frame, InterpDispatch, InterpreterError,
    SameStageLinker, SparseBackwardDriver, SparseBackwardInterpreter, StageQuery,
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

/// The dense backward engine instantiated at [`LiveSet`] point states:
/// classic per-program-point liveness. A language with structured dialects
/// supplies its own private stack-item `F` embedding the dialect's dense frames.
pub type DenseLiveness<
    'ir,
    S,
    E = InterpreterError,
    F = DenseBlockFrame<LiveSet, E>,
    Lk = SameStageLinker,
> = DenseBackwardInterpreter<'ir, S, LiveSet, E, F, Lk>;

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
    let mut engine = DemandAnalysis::<S>::new(pipeline);
    engine.analyze(stage, callee, result_values)?;
    DemandResult::from_engine(&engine)
}

/// Resolve `callee` and run classic per-point liveness over its body,
/// with the standard reverse block walker. Languages with structured dialects
/// select their stack-item type through
/// [`analyze_dense_with_frame`] instead.
pub fn analyze_dense<'ir, S>(
    pipeline: &'ir Pipeline<S>,
    stage: CompileStage,
    callee: Callee,
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
                DenseBlockFrame<LiveSet, InterpreterError>,
            >,
        >,
{
    analyze_dense_with_frame::<S, DenseBlockFrame<LiveSet, InterpreterError>>(
        pipeline, stage, callee,
    )
}

/// Resolve `callee` and run classic per-point liveness over its body with a
/// caller-selected stack-item type `F` — the entry point for
/// languages whose structured dialects require a language-specific private
/// composition. The analysis consumes the finalized IR directly; it neither
/// requires nor computes a demand ([`DemandResult`]) pre-pass.
pub fn analyze_dense_with_frame<'ir, S, F>(
    pipeline: &'ir Pipeline<S>,
    stage: CompileStage,
    callee: Callee,
) -> Result<DenseLivenessResult, InterpreterError>
where
    S: StageMeta
        + StageQuery
        + InterpDispatch<DenseBackwardTransfer<'ir, S, LiveSet, InterpreterError, F>>,
    F: Frame<
            DenseBackwardDriver<'ir, S, LiveSet, InterpreterError, F>,
            F,
            Completion = DenseBackwardCompletion<LiveSet>,
        > + From<DenseBlockFrame<LiveSet, InterpreterError>>,
{
    let mut engine = DenseLiveness::<S, InterpreterError, F>::new(pipeline);
    let scope = engine.analyze(stage, callee)?;
    Ok(DenseLivenessResult::from_engine(&engine, scope))
}
