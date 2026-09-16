use kirin_ir::{Body, CompileStage, Pipeline, SpecializedFunction, StageMeta};

use super::query;
use crate::{Callee, InterpreterError, StageQuery};

/// The stage and specialization selected by a linker.
///
/// The specialization record owns its definition statement. Body discovery
/// reads that record in `stage`, so a linker cannot supply a conflicting
/// definition alongside the specialization's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinkTarget {
    pub stage: CompileStage,
    pub specialization: SpecializedFunction,
}

impl LinkTarget {
    /// Discover this target's Kirin implementation in its own stage.
    ///
    /// Distinct from [`HasCallableBody::callable_body`](kirin_ir::HasCallableBody::callable_body),
    /// which projects a body from an *already available* dialect operation.
    /// This looks that operation up first — specialization record, then its
    /// authoritative definition statement — and only then projects. The
    /// lookup owns every failure mode the projection cannot express: a
    /// missing stage, specialization record, or statement.
    ///
    /// Each lookup reads the specialization's authoritative definition rather
    /// than retaining a separate body alongside the target.
    pub fn body<S: StageQuery>(&self, pipeline: &Pipeline<S>) -> Result<Body, InterpreterError> {
        query::callable_body(pipeline, self.stage, self.specialization)
    }
}

/// The calling-convention component of an engine.
///
/// A linker resolves a [`Callee`] to a [`LinkTarget`]. It is a value
/// passed to engines (`.with_linker(...)`), so compiler authors swap calling
/// conventions without touching engine internals — the same linker drives
/// concrete execution and abstract analyses, which is what makes
/// cross-language analysis a one-line choice.
pub trait Linker<S: StageMeta> {
    /// Resolve relative to `lookup_stage`; the selected target may live in
    /// another stage under a cross-stage policy.
    fn resolve(
        &self,
        pipeline: &Pipeline<S>,
        lookup_stage: CompileStage,
        callee: &Callee,
    ) -> Result<LinkTarget, InterpreterError>;
}

/// Resolve calls within the lookup stage only (the default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SameStageLinker;

/// Resolve calls across stages: prefer a live specialization at the lookup
/// stage, otherwise fall back to any stage that has one. This is the standard
/// linker for pipelines where functions are declared at several stages but
/// lowered bodies live at only one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CrossStageLinker;

fn callee_function<S: StageQuery>(
    pipeline: &Pipeline<S>,
    lookup_stage: CompileStage,
    callee: &Callee,
) -> Result<Callee, InterpreterError> {
    match *callee {
        Callee::Named(symbol) => {
            let name = query::resolve_symbol_name(pipeline, lookup_stage, symbol)?
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
) -> Result<LinkTarget, InterpreterError> {
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
    // In particular, an already specialized handle must exist here before
    // CrossStageLinker accepts this candidate stage.
    query::validate_specialization(pipeline, stage, specialized)?;
    Ok(LinkTarget {
        stage,
        specialization: specialized,
    })
}

impl<S: StageQuery> Linker<S> for SameStageLinker {
    fn resolve(
        &self,
        pipeline: &Pipeline<S>,
        lookup_stage: CompileStage,
        callee: &Callee,
    ) -> Result<LinkTarget, InterpreterError> {
        let callee = callee_function(pipeline, lookup_stage, callee)?;
        target_at_stage(pipeline, lookup_stage, &callee)
    }
}

impl<S: StageQuery> Linker<S> for CrossStageLinker {
    fn resolve(
        &self,
        pipeline: &Pipeline<S>,
        lookup_stage: CompileStage,
        callee: &Callee,
    ) -> Result<LinkTarget, InterpreterError> {
        let callee = callee_function(pipeline, lookup_stage, callee)?;
        let home = target_at_stage(pipeline, lookup_stage, &callee);
        if home.is_ok() {
            return home;
        }
        for stage in pipeline.stages().iter().filter_map(StageMeta::stage_id) {
            if stage == lookup_stage {
                continue;
            }
            if let Ok(target) = target_at_stage(pipeline, stage, &callee) {
                return Ok(target);
            }
        }
        home
    }
}
