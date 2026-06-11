//! Test-only directional fixpoint helpers.
//!
//! These exercise [`ConstPropFixpointInterpreter`] with explicit forward /
//! backward summary dependencies between a pair of lowered functions, to
//! verify that the directional dependency index produces the same final
//! summaries as the default driver.

use kirin::prelude::{CompileStage, Dialect, HasStageInfo, Pipeline, Symbol};
use kirin_constprop::{
    ConstPropFixpointInterpreter, ConstPropFunctionOwner, ConstPropOwner, ConstPropSummary,
    DefaultConstPropSemantics,
};
use kirin_function::interpreter::{CallTargetResolution, ResolvedCallTarget};
use kirin_interpreter::{
    AbstractEnvStore, BackwardSummaryDeps, ForwardSummaryDeps, FunctionEntryTarget,
    InterpreterError, Location, StageAccess, SummaryDependency, SummaryDependencyIndex,
};
use kirin_scf::ScfForConstPropSummary;

use crate::interpreter::{ConstProp, ToyError};
use crate::language::LowLevel;
use crate::stage::Stage;

use super::profile::ToyConstPropLowered;

type DirectionalFunctionFixpoint<'ir, Deps> =
    ConstPropFixpointInterpreter<'ir, ToyConstPropLowered, AbstractEnvStore<ConstProp>, Deps>;

impl<'ir, Deps> CallTargetResolution<LowLevel> for DirectionalFunctionFixpoint<'ir, Deps> {
    type Error = ToyError;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        resolve_current_stage_call_target::<LowLevel, _>(self, location, target)
    }
}

fn resolve_current_stage_call_target<L, Deps>(
    interp: &DirectionalFunctionFixpoint<'_, Deps>,
    location: Location,
    target: Symbol,
) -> Result<ResolvedCallTarget, ToyError>
where
    L: Dialect,
    Stage: HasStageInfo<L>,
{
    let stage = StageAccess::<L>::stage_info(interp, location.stage)?;
    let function = interp
        .pipeline()
        .resolve_function(stage, target)
        .ok_or(InterpreterError::MissingCallTarget { location, target })
        .map_err(ToyError::from)?;
    Ok(ResolvedCallTarget {
        stage: location.stage,
        target: FunctionEntryTarget::Function(function),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DirectionalConstPropResult {
    pub(super) source_value: ConstProp,
    pub(super) target_value: ConstProp,
    pub(super) source_visits: usize,
    pub(super) target_visits: usize,
}

pub(super) fn analyze_lowered_constprop_forward_dependencies(
    pipeline: &Pipeline<Stage>,
    source_name: &str,
    source_args: &[ConstProp],
    target_name: &str,
    target_args: &[ConstProp],
) -> Result<DirectionalConstPropResult, ToyError> {
    let stage = lowered_stage(pipeline)?;
    let source_owner = resolve_staged_owner(pipeline, stage, source_name)?;
    let target_owner = resolve_staged_owner(pipeline, stage, target_name)?;
    let mut deps = ForwardSummaryDeps::new();
    deps.register(
        &ConstPropOwner::Function(source_owner),
        SummaryDependency::Reanalyze(ConstPropOwner::Function(target_owner)),
    )?;
    run_lowered_directional_constprop(
        pipeline,
        deps,
        source_owner,
        target_owner,
        source_args,
        target_args,
        ConstPropOwner::Function(source_owner),
    )
}

pub(super) fn analyze_lowered_constprop_backward_dependencies(
    pipeline: &Pipeline<Stage>,
    predecessor_name: &str,
    predecessor_args: &[ConstProp],
    successor_name: &str,
    successor_args: &[ConstProp],
) -> Result<DirectionalConstPropResult, ToyError> {
    let stage = lowered_stage(pipeline)?;
    let predecessor_owner = resolve_staged_owner(pipeline, stage, predecessor_name)?;
    let successor_owner = resolve_staged_owner(pipeline, stage, successor_name)?;
    let mut deps = BackwardSummaryDeps::new();
    deps.register(
        &ConstPropOwner::Function(successor_owner),
        SummaryDependency::Reanalyze(ConstPropOwner::Function(predecessor_owner)),
    )?;
    run_lowered_directional_constprop(
        pipeline,
        deps,
        predecessor_owner,
        successor_owner,
        predecessor_args,
        successor_args,
        ConstPropOwner::Function(successor_owner),
    )
}

fn run_lowered_directional_constprop<Deps>(
    pipeline: &Pipeline<Stage>,
    deps: Deps,
    source: ConstPropFunctionOwner,
    target: ConstPropFunctionOwner,
    source_args: &[ConstProp],
    target_args: &[ConstProp],
    entry: ConstPropOwner,
) -> Result<DirectionalConstPropResult, ToyError>
where
    Deps: SummaryDependencyIndex<ConstPropOwner>,
    ToyError: From<Deps::Error>,
{
    let mut interp = DirectionalFunctionFixpoint::with_dependency_index(
        pipeline,
        AbstractEnvStore::new(),
        (),
        deps,
    );
    let mut semantics: DefaultConstPropSemantics<ConstProp, ScfForConstPropSummary<ConstProp>> =
        DefaultConstPropSemantics::empty()
            .with_args(source, source_args.iter().cloned())
            .with_args(target, target_args.iter().cloned());

    interp.solve(&mut semantics, entry)?;
    let source_value = interp
        .summary(&ConstPropOwner::Function(source))
        .and_then(ConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom);
    let target_value = interp
        .summary(&ConstPropOwner::Function(target))
        .and_then(ConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom);

    Ok(DirectionalConstPropResult {
        source_value,
        target_value,
        source_visits: semantics.visits(&ConstPropOwner::Function(source)),
        target_visits: semantics.visits(&ConstPropOwner::Function(target)),
    })
}

fn lowered_stage(pipeline: &Pipeline<Stage>) -> Result<CompileStage, ToyError> {
    pipeline
        .stage_by_name("lowered")
        .ok_or_else(|| ToyError::from(InterpreterError::Custom("missing lowered stage")))
}

fn resolve_staged_owner(
    pipeline: &Pipeline<Stage>,
    stage: CompileStage,
    function_name: &str,
) -> Result<ConstPropFunctionOwner, ToyError> {
    let function = pipeline
        .resolve_staged_function(function_name, stage)
        .ok_or(InterpreterError::Custom("missing staged function"))
        .map_err(ToyError::from)?;
    Ok(ConstPropFunctionOwner {
        stage,
        target: FunctionEntryTarget::StagedFunction(function),
    })
}
