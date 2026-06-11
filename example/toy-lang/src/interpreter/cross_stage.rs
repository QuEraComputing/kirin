//! Cross-stage call target resolution shared by the concrete and fixpoint
//! drivers.
//!
//! `kirin-function` ships a blanket `CallTargetResolution<L>` impl for
//! `ConcreteInterpreter` and `AbstractInterpreterWithStore` that resolves
//! calls within the caller's stage only. When a function is declared at
//! multiple stages but only has a body at one of them — the canonical
//! cross-stage scenario — that default cannot dispatch to the right body.
//!
//! This module provides the resolution logic both [`ToyConcreteInterpreter`]
//! (in `super::concrete`) and the const-prop fixpoint use: prefer a live
//! specialization at the caller's stage, fall back to any other stage that
//! has one, and return a generic `FunctionEntryTarget::Function` as the last
//! resort.

use kirin::prelude::{
    CompileStage, Dialect, Function, GetInfo, HasStageInfo, Pipeline, StageInfo, StageMeta, Symbol,
};
use kirin_function::interpreter::ResolvedCallTarget;
use kirin_interpreter::{FunctionEntryTarget, InterpreterError, Location};

use crate::stage::Stage;

use super::ToyError;

pub(super) fn resolve_cross_stage_call_target<L>(
    pipeline: &Pipeline<Stage>,
    location: Location,
    target: Symbol,
) -> Result<ResolvedCallTarget, ToyError>
where
    L: Dialect,
    Stage: HasStageInfo<L>,
{
    let stage_info: &StageInfo<L> = pipeline
        .stage(location.stage)
        .ok_or(InterpreterError::MissingStage(location.stage))
        .map_err(ToyError::from)?
        .try_stage_info()
        .ok_or(InterpreterError::MissingStageInfo(location.stage))
        .map_err(ToyError::from)?;
    let function = pipeline
        .resolve_function(stage_info, target)
        .ok_or(InterpreterError::MissingCallTarget { location, target })
        .map_err(ToyError::from)?;
    if let Some(target) = live_specialization_at_stage(pipeline, location.stage, function) {
        return Ok(target);
    }
    for stage in pipeline.stages().iter().filter_map(StageMeta::stage_id) {
        if stage == location.stage {
            continue;
        }
        if let Some(target) = live_specialization_at_stage(pipeline, stage, function) {
            return Ok(target);
        }
    }
    Ok(ResolvedCallTarget {
        stage: location.stage,
        target: FunctionEntryTarget::Function(function),
    })
}

fn live_specialization_at_stage(
    pipeline: &Pipeline<Stage>,
    stage: CompileStage,
    function: Function,
) -> Option<ResolvedCallTarget> {
    let staged = pipeline.function_info(function)?.staged_function(stage)?;
    let specialized = match pipeline.stage(stage)? {
        Stage::Source(info) => staged.get_info(info)?.unique_live_specialization().ok()?,
        Stage::Lowered(info) => staged.get_info(info)?.unique_live_specialization().ok()?,
    };
    Some(ResolvedCallTarget {
        stage,
        target: FunctionEntryTarget::SpecializedFunction(specialized),
    })
}
