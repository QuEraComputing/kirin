//! Classic SSA liveness for Kirin: a [`LiveSet`] at every program point (the
//! regalloc-grade fact), computed by the dense backward engine
//! ([`kirin_interpreter::DenseBackwardInterpreter`]) through
//! [`analyze_dense`]. Every use gens and every definition kills; purity is
//! irrelevant, so dead code's operands appear in these sets. Transfer comes
//! from each dialect's `Interpretable<I, ClassicLiveness>` rule.
//!
//! Strong liveness (which values are transitively needed by a root) is the
//! separate sparse backward analysis in `kirin-demand`.
//!
//! ```ignore
//! let result = kirin_liveness::analyze_dense(&pipeline, stage, callee)?;
//! let live = result.point_facts(point);
//! ```

mod live;
mod result;

pub use live::LiveSet;
pub use result::DenseLivenessResult;

use kirin_interpreter::{
    Callee, DenseBackwardCompletion, DenseBackwardDriver, DenseBackwardInterpreter,
    DenseBackwardTransfer, DenseBlockFrame, Frame, InterpDispatch, InterpreterError,
    SameStageLinker, StageQuery,
};
use kirin_ir::{CompileStage, Pipeline, StageMeta};

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
/// requires nor computes a strong-liveness pre-pass.
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
