use kirin_ir::{CompileStage, Pipeline, SpecializedFunction, StageMeta, Statement};

use super::query;
use crate::{CallableBody, Callee, Interp, InterpDispatch, InterpreterError, StageQuery};

/// A fully resolved call target: the stage to execute in, the specialization,
/// and its callable definition statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionTarget {
    pub stage: CompileStage,
    pub function: SpecializedFunction,
    pub definition: Statement,
}

/// The calling-convention component of an engine.
///
/// A linker resolves a [`Callee`] to a [`FunctionTarget`]. It is a value
/// passed to engines (`.with_linker(...)`), so compiler authors swap calling
/// conventions without touching engine internals — the same linker drives
/// concrete execution and abstract analyses, which is what makes
/// cross-language analysis a one-line choice.
pub trait Linker<S: StageMeta> {
    fn resolve(
        &self,
        pipeline: &Pipeline<S>,
        caller_stage: CompileStage,
        callee: &Callee,
    ) -> Result<FunctionTarget, InterpreterError>;
}

/// Run the framework's common callable-root protocol.
///
/// Linking selects a concrete target; callable-entry dispatch then discovers
/// its body in the target's stage. Engines invoke this operation before
/// applying their own boundary inputs (runtime arguments, abstract arguments,
/// or analysis-specific seeds).
pub(crate) fn resolve_callable<I, S, Lk>(
    pipeline: &Pipeline<S>,
    linker: &Lk,
    caller_stage: CompileStage,
    callee: &Callee,
) -> Result<(FunctionTarget, CallableBody), I::Error>
where
    I: Interp,
    S: StageMeta + InterpDispatch<I>,
    Lk: Linker<S>,
{
    let target = linker
        .resolve(pipeline, caller_stage, callee)
        .map_err(I::Error::from)?;
    let info = pipeline
        .stage(target.stage)
        .ok_or_else(|| I::Error::from(InterpreterError::MissingStage(target.stage)))?;
    let body = info.dispatch_function_entry(target.definition)?;
    Ok((target, body))
}

/// Resolve calls within the caller's stage only (the default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SameStageLinker;

/// Resolve calls across stages: prefer a live specialization at the caller's
/// stage, otherwise fall back to any stage that has one. This is the standard
/// linker for pipelines where functions are declared at several stages but
/// lowered bodies live at only one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CrossStageLinker;

fn callee_function<S: StageQuery>(
    pipeline: &Pipeline<S>,
    caller_stage: CompileStage,
    callee: &Callee,
) -> Result<Callee, InterpreterError> {
    match *callee {
        Callee::Named(symbol) => {
            let name = query::resolve_symbol_name(pipeline, caller_stage, symbol)?
                .ok_or(InterpreterError::MissingCallSymbol(symbol))?;
            let function = pipeline
                .lookup_function_by_name(&name)
                .ok_or(InterpreterError::MissingFunctionName(name))?;
            Ok(Callee::Function(function))
        }
        other => Ok(other),
    }
}

/// Resolve a (symbol-free) callee at a specific stage.
fn target_at_stage<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    callee: &Callee,
) -> Result<FunctionTarget, InterpreterError> {
    let specialized = match *callee {
        Callee::Named(symbol) => return Err(InterpreterError::MissingCallSymbol(symbol)),
        Callee::Function(function) => {
            let staged = pipeline
                .function_info(function)
                .ok_or(InterpreterError::MissingFunction(function))?
                .staged_function(stage)
                .ok_or(InterpreterError::MissingStagedFunction { function, stage })?;
            query::unique_specialization(pipeline, stage, staged)?
        }
        Callee::Staged(staged) => query::unique_specialization(pipeline, stage, staged)?,
        Callee::Specialized(specialized) => specialized,
    };
    let definition = query::function_definition(pipeline, stage, specialized)?;
    Ok(FunctionTarget {
        stage,
        function: specialized,
        definition,
    })
}

impl<S: StageQuery> Linker<S> for SameStageLinker {
    fn resolve(
        &self,
        pipeline: &Pipeline<S>,
        caller_stage: CompileStage,
        callee: &Callee,
    ) -> Result<FunctionTarget, InterpreterError> {
        let callee = callee_function(pipeline, caller_stage, callee)?;
        target_at_stage(pipeline, caller_stage, &callee)
    }
}

impl<S: StageQuery> Linker<S> for CrossStageLinker {
    fn resolve(
        &self,
        pipeline: &Pipeline<S>,
        caller_stage: CompileStage,
        callee: &Callee,
    ) -> Result<FunctionTarget, InterpreterError> {
        let callee = callee_function(pipeline, caller_stage, callee)?;
        let home = target_at_stage(pipeline, caller_stage, &callee);
        if home.is_ok() {
            return home;
        }
        for stage in pipeline.stages().iter().filter_map(StageMeta::stage_id) {
            if stage == caller_stage {
                continue;
            }
            if let Ok(target) = target_at_stage(pipeline, stage, &callee) {
                return Ok(target);
            }
        }
        home
    }
}
