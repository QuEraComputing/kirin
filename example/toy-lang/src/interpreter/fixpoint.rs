use kirin::prelude::{CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, StageMeta, Symbol};
use kirin_constprop::{ConstPropDriver, ConstPropFunctionFixpoint};
#[cfg(test)]
use kirin_constprop::{
    ConstPropFixpointInterpreter, ConstPropFunctionOwner, ConstPropOwner, DefaultConstPropSemantics,
};
use kirin_function::interpreter::{CallTargetResolution, ResolvedCallTarget};
#[cfg(test)]
use kirin_interpreter::FunctionInvocation;
use kirin_interpreter::{
    AbstractEnvStore, FunctionEntryTarget, InterpreterError, Location, StageAccess,
};
#[cfg(test)]
use kirin_interpreter::{BackwardSummaryDeps, ForwardSummaryDeps};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

#[cfg(test)]
use kirin_scf::ScfForConstPropSummary;

use super::profile::ToyConstPropFunction;
#[cfg(test)]
use super::profile::ToyConstPropLowered;
use super::{ConstProp, ToyError};

type FunctionFixpoint<'ir> = ConstPropFunctionFixpoint<'ir, ToyConstPropFunction>;

#[cfg(test)]
type DirectionalFunctionFixpoint<'ir, Deps> =
    ConstPropFixpointInterpreter<'ir, ToyConstPropLowered, AbstractEnvStore<ConstProp>, Deps>;

impl<'ir> CallTargetResolution<HighLevel> for FunctionFixpoint<'ir> {
    type Error = ToyError;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        resolve_cross_stage_call_target::<HighLevel>(self, location, target)
    }
}

impl<'ir> CallTargetResolution<LowLevel> for FunctionFixpoint<'ir> {
    type Error = ToyError;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        resolve_cross_stage_call_target::<LowLevel>(self, location, target)
    }
}

#[cfg(test)]
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

#[cfg(test)]
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

fn resolve_cross_stage_call_target<L>(
    interp: &FunctionFixpoint<'_>,
    location: Location,
    target: Symbol,
) -> Result<ResolvedCallTarget, ToyError>
where
    Stage: HasStageInfo<L>,
    L: Dialect,
{
    let stage = StageAccess::<L>::stage_info(interp, location.stage)?;
    let function = interp
        .pipeline()
        .resolve_function(stage, target)
        .ok_or(InterpreterError::MissingCallTarget { location, target })
        .map_err(ToyError::from)?;
    if let Some(target) = live_specialization_at_stage(interp.pipeline(), location.stage, function)
    {
        return Ok(target);
    }
    for stage in interp
        .pipeline()
        .stages()
        .iter()
        .filter_map(StageMeta::stage_id)
    {
        if stage == location.stage {
            continue;
        }
        if let Some(target) = live_specialization_at_stage(interp.pipeline(), stage, function) {
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
    function: kirin::prelude::Function,
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

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectionalConstPropResult {
    pub(crate) source_value: ConstProp,
    pub(crate) target_value: ConstProp,
    pub(crate) source_visits: usize,
    pub(crate) target_visits: usize,
}

#[cfg(test)]
pub(crate) fn analyze_source_constprop_invocation(
    pipeline: &Pipeline<Stage>,
    invocation: FunctionInvocation<ConstProp>,
) -> Result<ConstProp, ToyError> {
    let mut interp: FunctionFixpoint<'_> =
        ConstPropFunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    interp.analyze_function(
        invocation.stage(),
        invocation.target(),
        invocation.args().iter().cloned(),
    )
}

pub fn analyze_source_constprop_fixpoint(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let mut interp: FunctionFixpoint<'_> =
        ConstPropFunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    interp.analyze_function_by_name("source", function_name, args.iter().cloned())
}

#[cfg(test)]
pub(crate) fn analyze_lowered_constprop_forward_dependencies(
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
    use kirin_interpreter::SummaryDependency;
    use kirin_interpreter::SummaryDependencyIndex;
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

#[cfg(test)]
pub(crate) fn analyze_lowered_constprop_backward_dependencies(
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
    use kirin_interpreter::SummaryDependency;
    use kirin_interpreter::SummaryDependencyIndex;
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

#[cfg(test)]
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
    Deps: kirin_interpreter::SummaryDependencyIndex<ConstPropOwner>,
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
        .and_then(kirin_constprop::ConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom);
    let target_value = interp
        .summary(&ConstPropOwner::Function(target))
        .and_then(kirin_constprop::ConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom);

    Ok(DirectionalConstPropResult {
        source_value,
        target_value,
        source_visits: semantics.visits(&ConstPropOwner::Function(source)),
        target_visits: semantics.visits(&ConstPropOwner::Function(target)),
    })
}

#[cfg(test)]
fn lowered_stage(pipeline: &Pipeline<Stage>) -> Result<CompileStage, ToyError> {
    pipeline.stage_by_name("lowered").ok_or_else(|| {
        ToyError::from(kirin_interpreter::InterpreterError::Custom(
            "missing lowered stage",
        ))
    })
}

#[cfg(test)]
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

pub fn analyze_lowered_constprop_fixpoint(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let mut interp: FunctionFixpoint<'_> =
        ConstPropFunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    interp.analyze_function_by_name("lowered", function_name, args.iter().cloned())
}
