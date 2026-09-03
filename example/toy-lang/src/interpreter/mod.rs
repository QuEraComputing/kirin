//! Interpreters and analyses for the toy language.
//!
//! All execution machinery lives in `kirin-interpreter`; this module only
//! picks value types, error lifting, and linkers. The cross-language behavior
//! (source-stage functions calling lowered-stage-only functions and back) is
//! a linker choice: [`CrossStageLinker`].

mod error;
mod frame;

#[cfg(test)]
mod tests;

pub use error::ToyError;
use frame::FrameStackItem;
pub use frame::ToyAbstractFrame;
pub(crate) use frame::ToyDenseBackwardFrame;

use kirin::prelude::{CFG, CompileStage, Pipeline};
use kirin_constprop::{ConstPropContext, ConstPropValue};
use kirin_interpreter::engine::{
    CallContext, ConcreteInterpreterCore, CrossStageLinker, Linker, SameStageLinker,
    SparseForwardInterpreter, expect_single,
};
use kirin_interpreter::{Body, Callee, InterpreterError};
use kirin_liveness::{DenseLiveness, DenseLivenessResult, LiveSet};

use crate::stage::Stage;

/// Summary key of the constant-propagation analysis policy.
type CpKey = <ConstPropContext as CallContext<ConstPropValue>>::Key;

type ToyEngine<'ir, Lk> =
    ConcreteInterpreterCore<'ir, Stage, i64, ToyError, Lk, FrameStackItem<i64, ToyError>>;

/// Concrete toy-language interpreter. Its explicitly declared frame-stack-item enum
/// includes the dialect-owned SCF continuations but remains private behind
/// this wrapper.
pub struct ToyInterpreter<'ir, Lk = CrossStageLinker> {
    inner: ToyEngine<'ir, Lk>,
}

impl<'ir> ToyInterpreter<'ir, CrossStageLinker> {
    pub fn new(pipeline: &'ir Pipeline<Stage>) -> Self {
        Self {
            inner: ConcreteInterpreterCore::new(pipeline).with_linker(CrossStageLinker),
        }
    }
}

impl<'ir> ToyInterpreter<'ir, SameStageLinker> {
    fn same_stage(pipeline: &'ir Pipeline<Stage>) -> Self {
        Self {
            inner: ConcreteInterpreterCore::new(pipeline),
        }
    }
}

impl<'ir, Lk> ToyInterpreter<'ir, Lk>
where
    Lk: Linker<Stage>,
{
    pub fn call_by_name(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: impl IntoIterator<Item = i64>,
    ) -> Result<kirin::prelude::Product<i64>, ToyError> {
        self.inner.call_by_name(stage_name, function_name, args)
    }
}

/// Cross-language constant propagation, with a frame type embedding the SCF
/// loop frame.
pub type ToyConstProp<'ir, Lk = CrossStageLinker> = SparseForwardInterpreter<
    'ir,
    Stage,
    ConstPropValue,
    ToyError,
    Lk,
    ConstPropContext,
    ToyAbstractFrame<ConstPropValue, ToyError, CpKey>,
>;

/// Execute `function_name` starting at `stage_name`, following calls across
/// language boundaries.
pub fn run_i64(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp = ToyInterpreter::new(pipeline);
    expect_single(interp.call_by_name(stage_name, function_name, args.iter().copied())?)
}

/// Execute within the source stage only.
pub fn run_source_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    run_same_stage_i64(pipeline, "source", function_name, args)
}

/// Execute within the lowered stage only.
pub fn run_lowered_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    run_same_stage_i64(pipeline, "lowered", function_name, args)
}

fn run_same_stage_i64(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp = ToyInterpreter::same_stage(pipeline);
    expect_single(interp.call_by_name(stage_name, function_name, args.iter().copied())?)
}

/// Run constant propagation from `function_name` at `stage_name`, following
/// calls across language boundaries. Returns the function's inferred return
/// value at the fixpoint.
pub fn analyze_constprop(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
    args: &[ConstPropValue],
) -> Result<ConstPropValue, ToyError> {
    let mut analysis: ToyConstProp<'_> = ToyConstProp::new(pipeline).with_linker(CrossStageLinker);
    expect_single(analysis.analyze_by_name(stage_name, function_name, args.iter().cloned())?)
}

/// Run classic per-point liveness (dense backward — regalloc-grade
/// block-boundary and per-statement sets) over `function_name`'s body at
/// `stage_name`. Consumes the finalized IR directly; strong demand
/// ([`kirin_liveness::analyze_demand`]) is an independent analysis and is
/// not involved.
pub fn analyze_classic_liveness(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
) -> Result<(CompileStage, CFG, DenseLivenessResult), InterpreterError> {
    let caller_stage = pipeline
        .stage_by_name(stage_name)
        .ok_or_else(|| InterpreterError::MissingStageName(stage_name.into()))?;
    let function = pipeline
        .lookup_function_by_name(function_name)
        .ok_or_else(|| InterpreterError::MissingFunctionName(function_name.into()))?;
    let mut engine: DenseLiveness<
        '_,
        Stage,
        InterpreterError,
        ToyDenseBackwardFrame<LiveSet, InterpreterError>,
        CrossStageLinker,
    > = DenseLiveness::new(pipeline).with_linker(CrossStageLinker);
    let scope = engine.analyze(caller_stage, Callee::Function(function))?;
    let result = DenseLivenessResult::from_engine(&engine, scope);
    let (stage, Body::CFG(cfg)) = scope else {
        return Err(InterpreterError::Custom(
            "classic liveness target is not a CFG function",
        ));
    };
    Ok((stage, cfg, result))
}
