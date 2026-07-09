use kirin_ir::{CompileStage, Pipeline, SpecializedFunction, StageMeta, Statement};

use crate::{Callee, InterpreterError, StageQuery, query};

/// A fully resolved call target: the stage to execute in, the specialization,
/// and its body statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionTarget {
    pub stage: CompileStage,
    pub function: SpecializedFunction,
    pub body: Statement,
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
                .ok_or(InterpreterError::MissingCallTarget(symbol))?;
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
        Callee::Named(symbol) => return Err(InterpreterError::MissingCallTarget(symbol)),
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
    let body = query::function_body(pipeline, stage, specialized)?;
    Ok(FunctionTarget {
        stage,
        function: specialized,
        body,
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
