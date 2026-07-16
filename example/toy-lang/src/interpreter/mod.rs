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
pub use frame::{ToyAbstractFrame, ToyDenseBackwardFrame, ToyFrame};

use kirin::prelude::{Cfg, CompileStage, GetInfo, Pipeline, UniqueLiveSpecializationError};
use kirin_constprop::{ConstPropContext, ConstPropValue};
use kirin_function::{Lexical, Lifted};
use kirin_interpreter::InterpreterError;
use kirin_interpreter::engine::{
    CallContext, ConcreteInterpreter, CrossStageLinker, SameStageLinker, SparseForwardInterpreter,
    expect_single,
};
use kirin_liveness::{DenseLivenessResult, LiveSet};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

/// Summary key of the constant-propagation analysis policy.
type CpKey = <ConstPropContext as CallContext<ConstPropValue>>::Key;

/// Concrete cross-language interpreter over machine integers. Its frame type
/// embeds the SCF loop frame (the toy language uses `scf.for`).
pub type ToyInterpreter<'ir, Lk = CrossStageLinker> =
    ConcreteInterpreter<'ir, Stage, i64, ToyError, Lk, ToyFrame<i64, ToyError>>;

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
    let mut interp: ToyInterpreter<'_> =
        ConcreteInterpreter::new(pipeline).with_linker(CrossStageLinker);
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
    let mut interp: ToyInterpreter<'_, SameStageLinker> = ConcreteInterpreter::new(pipeline);
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

/// The body cfg of `function_name`'s specialization at `stage_name`.
fn function_cfg(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
) -> Result<(CompileStage, Cfg), InterpreterError> {
    let stage_id = pipeline
        .stage_by_name(stage_name)
        .ok_or_else(|| InterpreterError::MissingStageName(stage_name.into()))?;
    let staged = pipeline
        .resolve_staged_function(function_name, stage_id)
        .ok_or_else(|| InterpreterError::MissingFunctionName(function_name.into()))?;
    let stage = pipeline
        .stage(stage_id)
        .ok_or(InterpreterError::MissingStage(stage_id))?;

    let cfg = match stage {
        Stage::Source(info) => {
            let staged_info = staged
                .get_info(info)
                .ok_or(InterpreterError::MissingSpecialization(staged))?;
            let spec = match staged_info.unique_live_specialization() {
                Ok(spec) => spec,
                Err(UniqueLiveSpecializationError::NoSpecialization) => {
                    return Err(InterpreterError::MissingSpecialization(staged));
                }
                Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                    return Err(InterpreterError::AmbiguousSpecialization {
                        function: staged,
                        count,
                    });
                }
            };
            let spec_info = spec
                .get_info(info)
                .ok_or(InterpreterError::Custom("specialized function has no body"))?;
            match spec_info.body().definition(info) {
                HighLevel::Lexical(Lexical::Function(function)) => {
                    use kirin::prelude::HasCfgBody;
                    *function.cfg()
                }
                _ => return Err(InterpreterError::Custom("expected a function body")),
            }
        }
        Stage::Lowered(info) => {
            let staged_info = staged
                .get_info(info)
                .ok_or(InterpreterError::MissingSpecialization(staged))?;
            let spec = match staged_info.unique_live_specialization() {
                Ok(spec) => spec,
                Err(UniqueLiveSpecializationError::NoSpecialization) => {
                    return Err(InterpreterError::MissingSpecialization(staged));
                }
                Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                    return Err(InterpreterError::AmbiguousSpecialization {
                        function: staged,
                        count,
                    });
                }
            };
            let spec_info = spec
                .get_info(info)
                .ok_or(InterpreterError::Custom("specialized function has no body"))?;
            match spec_info.body().definition(info) {
                LowLevel::Lifted(Lifted::Function(function)) => {
                    use kirin::prelude::HasCfgBody;
                    *function.cfg()
                }
                _ => return Err(InterpreterError::Custom("expected a function body")),
            }
        }
    };
    Ok((stage_id, cfg))
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
) -> Result<DenseLivenessResult, InterpreterError> {
    let (stage, cfg) = function_cfg(pipeline, stage_name, function_name)?;
    kirin_liveness::analyze_dense_with_frame::<_, ToyDenseBackwardFrame<LiveSet, InterpreterError>>(
        pipeline, stage, cfg,
    )
}
