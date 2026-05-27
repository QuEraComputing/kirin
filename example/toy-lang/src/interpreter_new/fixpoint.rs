#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use kirin::prelude::TryLiftFrom;
use kirin::prelude::{
    CompileStage, Dialect, GetInfo, HasStageInfo, LiftFrom, Pipeline, Product, StageMeta, Symbol,
    TryLift,
};
#[cfg(test)]
use kirin_constprop::ConstPropFunctionOwner;
use kirin_constprop::{ConstPropFixpointInterpreter, ConstPropOwner};
use kirin_function::interpreter_new::{CallTargetResolution, ResolvedCallTarget};
use kirin_interpreter_new::{
    AbstractBlockTransfer, AbstractEnvStore, BlockFrame, Env, FunctionEntryTarget,
    FunctionInvocation, FunctionInvocationDispatch, InterpreterError, Location, OwnerSemantics,
    StageAccess, SummaryEffect,
};
#[cfg(test)]
use kirin_interpreter_new::{
    BackwardSummaryDeps, ForwardSummaryDeps, FunctionFrame, SpecializedFunctionFrame,
    StagedFunctionFrame, SummaryDependency, SummaryDependencyIndex,
};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

use kirin_scf::interpreter_new::ScfCompletion;
use kirin_scf::{ForLoopValue, ScfForConstPropSummary};

#[cfg(test)]
use super::ToyFrame;
use super::run::expect_function_return;
use super::run::resolve_function;
use super::{ConstProp, ToyCompletion, ToyError, ToyStageFrame};

fn expect_scf_yield(completion: ToyCompletion<ConstProp>) -> Result<Product<ConstProp>, ToyError> {
    match completion {
        ToyCompletion::Scf(ScfCompletion::Yield(value)) => Ok(value),
        _ => Err(ToyError::lift_from(InterpreterError::Custom(
            "expected scf.yield",
        ))),
    }
}

struct ConstPropSemantics {
    args: Vec<ConstProp>,
}

impl ConstPropSemantics {
    fn new(args: &[ConstProp]) -> Self {
        Self {
            args: args.to_vec(),
        }
    }
}

type FunctionFixpoint<'ir> = ConstPropFixpointInterpreter<
    'ir,
    Stage,
    ConstPropOwner,
    ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>,
    ToyCompletion<ConstProp>,
    ToyError,
    ToyConstPropSummary,
>;

type ToyConstPropSummary =
    kirin_constprop::ConstPropSummary<ConstProp, ScfForConstPropSummary<ConstProp>>;

#[cfg(test)]
type DirectionalFunctionFixpoint<'ir, Deps> = ConstPropFixpointInterpreter<
    'ir,
    Stage,
    ConstPropOwner,
    ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
    ToyCompletion<ConstProp>,
    ToyError,
    ToyConstPropSummary,
    AbstractEnvStore<ConstProp>,
    Deps,
>;

impl<'ir>
    FunctionInvocationDispatch<
        ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyError,
        ConstProp,
    > for FunctionFixpoint<'ir>
{
    fn dispatch_function_invocation(
        &mut self,
        invocation: FunctionInvocation<ConstProp>,
    ) -> Result<ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>, ToyError> {
        match self.pipeline().stage(invocation.stage()) {
            Some(Stage::Source(_)) => invocation.into_root_frame::<HighLevel, _, ToyError>(),
            Some(Stage::Lowered(_)) => invocation.into_root_frame::<LowLevel, _, ToyError>(),
            None => Err(ToyError::lift_from(InterpreterError::MissingStage(
                invocation.stage(),
            ))),
        }
    }
}

#[cfg(test)]
impl<'ir, Deps>
    FunctionInvocationDispatch<
        ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyError,
        ConstProp,
    > for DirectionalFunctionFixpoint<'ir, Deps>
{
    fn dispatch_function_invocation(
        &mut self,
        invocation: FunctionInvocation<ConstProp>,
    ) -> Result<ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>, ToyError> {
        invocation.into_root_frame::<LowLevel, _, ToyError>()
    }
}

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
        .map_err(ToyError::lift_from)?;
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
        .map_err(ToyError::lift_from)?;
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

impl<'ir>
    OwnerSemantics<
        FunctionFixpoint<'ir>,
        ConstPropOwner,
        ToyConstPropSummary,
        ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
    > for ConstPropSemantics
{
    fn bottom_summary(
        &mut self,
        _interp: &mut FunctionFixpoint<'ir>,
        owner: &ConstPropOwner,
    ) -> Result<ToyConstPropSummary, ToyError> {
        Ok(match owner {
            ConstPropOwner::Function(_) => ToyConstPropSummary::function_bottom(),
            ConstPropOwner::Location(_) => ToyConstPropSummary::location_bottom(),
        })
    }

    fn entry_frame(
        &mut self,
        interp: &mut FunctionFixpoint<'ir>,
        owner: &ConstPropOwner,
        summary: &ToyConstPropSummary,
    ) -> Result<ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>, ToyError> {
        match owner {
            ConstPropOwner::Function(owner) => {
                interp.dispatch_function_invocation(FunctionInvocation::new(
                    owner.stage,
                    owner.target,
                    Product::from_vec(self.args.clone()),
                ))
            }
            ConstPropOwner::Location(_) => {
                let state = summary.location_state().ok_or_else(|| {
                    ToyError::lift_from(InterpreterError::Custom("missing scf.for summary state"))
                })?;
                let env = interp.alloc();
                match interp.pipeline().stage(owner.stage()) {
                    Some(Stage::Source(_)) => BlockFrame::<
                        HighLevel,
                        ConstProp,
                        AbstractBlockTransfer<ConstProp>,
                    >::new(
                        owner.stage(), state.body, env, state.body_args()
                    )
                    .try_lift()
                    .map_err(ToyError::from),
                    Some(Stage::Lowered(_)) => Err(ToyError::lift_from(InterpreterError::Custom(
                        "scf.for fixpoint owner cannot run at lowered stage",
                    ))),
                    None => Err(ToyError::lift_from(InterpreterError::MissingStage(
                        owner.stage(),
                    ))),
                }
            }
        }
    }

    fn complete_owner(
        &mut self,
        interp: &mut FunctionFixpoint<'ir>,
        owner: ConstPropOwner,
        completion: ToyCompletion<ConstProp>,
    ) -> Result<SummaryEffect<ConstPropOwner, ToyConstPropSummary>, ToyError> {
        match owner {
            ConstPropOwner::Function(_) => Ok(SummaryEffect::Update {
                owner,
                candidate: ToyConstPropSummary::function(expect_function_return(completion)?),
            }),
            ConstPropOwner::Location(_) => {
                let current = interp
                    .summary(&owner)
                    .and_then(ToyConstPropSummary::location_state)
                    .cloned()
                    .ok_or_else(|| {
                        ToyError::lift_from(InterpreterError::Custom(
                            "missing scf.for summary during completion",
                        ))
                    })?;
                let carried = expect_scf_yield(completion)?;
                let next = current
                    .advance_with(carried, ForLoopValue::loop_step)
                    .ok_or_else(|| ToyError::lift_from(InterpreterError::LoopStepOverflow))?;
                Ok(SummaryEffect::Update {
                    owner,
                    candidate: ToyConstPropSummary::location(next),
                })
            }
        }
    }
}

#[cfg(test)]
struct DirectionalConstPropSemantics {
    args_by_owner: HashMap<ConstPropOwner, Vec<ConstProp>>,
    visits_by_owner: HashMap<ConstPropOwner, usize>,
}

#[cfg(test)]
impl DirectionalConstPropSemantics {
    fn new(entries: impl IntoIterator<Item = (ConstPropOwner, Vec<ConstProp>)>) -> Self {
        Self {
            args_by_owner: entries.into_iter().collect(),
            visits_by_owner: HashMap::new(),
        }
    }

    fn visits(&self, owner: ConstPropOwner) -> usize {
        self.visits_by_owner.get(&owner).copied().unwrap_or(0)
    }
}

#[cfg(test)]
impl<'ir, Deps>
    OwnerSemantics<
        DirectionalFunctionFixpoint<'ir, Deps>,
        ConstPropOwner,
        ToyConstPropSummary,
        ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
    > for DirectionalConstPropSemantics
where
    ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>: TryLiftFrom<FunctionFrame<LowLevel, ConstProp>>
        + TryLiftFrom<StagedFunctionFrame<LowLevel, ConstProp>>
        + TryLiftFrom<SpecializedFunctionFrame<LowLevel, ConstProp>>,
    ToyError: From<
            <ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>> as TryLiftFrom<
                FunctionFrame<LowLevel, ConstProp>,
            >>::Error,
        > + From<
            <ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>> as TryLiftFrom<
                StagedFunctionFrame<LowLevel, ConstProp>,
            >>::Error,
        > + From<
            <ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>> as TryLiftFrom<
                SpecializedFunctionFrame<LowLevel, ConstProp>,
            >>::Error,
        >,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut DirectionalFunctionFixpoint<'ir, Deps>,
        owner: &ConstPropOwner,
    ) -> Result<ToyConstPropSummary, ToyError> {
        Ok(match owner {
            ConstPropOwner::Function(_) => ToyConstPropSummary::function_bottom(),
            ConstPropOwner::Location(_) => ToyConstPropSummary::location_bottom(),
        })
    }

    fn entry_frame(
        &mut self,
        _interp: &mut DirectionalFunctionFixpoint<'ir, Deps>,
        owner: &ConstPropOwner,
        _summary: &ToyConstPropSummary,
    ) -> Result<ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>, ToyError> {
        *self.visits_by_owner.entry(owner.clone()).or_default() += 1;
        let args = self.args_by_owner.get(owner).cloned().ok_or_else(|| {
            ToyError::lift_from(InterpreterError::Custom(
                "missing directional const-prop args",
            ))
        })?;
        let ConstPropOwner::Function(owner) = owner else {
            return Err(ToyError::lift_from(InterpreterError::Custom(
                "directional const-prop expected function owner",
            )));
        };
        FunctionInvocation::new(owner.stage, owner.target, Product::from_vec(args))
            .into_root_frame()
    }

    fn complete_owner(
        &mut self,
        _interp: &mut DirectionalFunctionFixpoint<'ir, Deps>,
        owner: ConstPropOwner,
        completion: ToyCompletion<ConstProp>,
    ) -> Result<SummaryEffect<ConstPropOwner, ToyConstPropSummary>, ToyError> {
        Ok(SummaryEffect::Update {
            owner,
            candidate: ToyConstPropSummary::function(expect_function_return(completion)?),
        })
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectionalConstPropResult {
    pub(crate) source_value: ConstProp,
    pub(crate) target_value: ConstProp,
    pub(crate) source_visits: usize,
    pub(crate) target_visits: usize,
}

pub fn analyze_source_constprop_invocation(
    pipeline: &Pipeline<Stage>,
    invocation: FunctionInvocation<ConstProp>,
) -> Result<ConstProp, ToyError> {
    let owner = ConstPropOwner::function(invocation.stage(), invocation.target());
    let args = invocation.args().iter().cloned().collect::<Vec<_>>();
    let mut interp: FunctionFixpoint<'_> =
        FunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    let mut semantics = ConstPropSemantics::new(&args);

    interp.solve(&mut semantics, owner)?;
    Ok(interp
        .summary(&owner)
        .and_then(ToyConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom))
}

pub fn analyze_source_constprop_fixpoint(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let stage = match pipeline.stage_by_name("source") {
        Some(stage) => stage,
        None => {
            return Err(ToyError::lift_from(
                kirin_interpreter_new::InterpreterError::Custom("missing source stage"),
            ));
        }
    };
    let function = resolve_function(pipeline, function_name)?;
    analyze_source_constprop_invocation(
        pipeline,
        FunctionInvocation::function(stage, function, Product::from_vec(args.to_vec())),
    )
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
    let source = ConstPropOwner::Function(resolve_staged_owner(pipeline, stage, source_name)?);
    let target = ConstPropOwner::Function(resolve_staged_owner(pipeline, stage, target_name)?);
    let mut deps = ForwardSummaryDeps::new();
    deps.register(&source, SummaryDependency::Reanalyze(target))?;
    run_lowered_directional_constprop(
        pipeline,
        deps,
        source,
        target,
        source_args,
        target_args,
        source,
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
    let predecessor =
        ConstPropOwner::Function(resolve_staged_owner(pipeline, stage, predecessor_name)?);
    let successor =
        ConstPropOwner::Function(resolve_staged_owner(pipeline, stage, successor_name)?);
    let mut deps = BackwardSummaryDeps::new();
    deps.register(&successor, SummaryDependency::Reanalyze(predecessor))?;
    run_lowered_directional_constprop(
        pipeline,
        deps,
        predecessor,
        successor,
        predecessor_args,
        successor_args,
        successor,
    )
}

#[cfg(test)]
fn run_lowered_directional_constprop<Deps>(
    pipeline: &Pipeline<Stage>,
    deps: Deps,
    source: ConstPropOwner,
    target: ConstPropOwner,
    source_args: &[ConstProp],
    target_args: &[ConstProp],
    entry: ConstPropOwner,
) -> Result<DirectionalConstPropResult, ToyError>
where
    Deps: SummaryDependencyIndex<ConstPropOwner>,
    ToyError: LiftFrom<Deps::Error>,
{
    let mut interp = DirectionalFunctionFixpoint::with_dependency_index(
        pipeline,
        AbstractEnvStore::new(),
        (),
        deps,
    );
    let mut semantics = DirectionalConstPropSemantics::new([
        (source, source_args.to_vec()),
        (target, target_args.to_vec()),
    ]);

    interp.solve(&mut semantics, entry)?;
    let source_value = interp
        .summary(&source)
        .and_then(ToyConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom);
    let target_value = interp
        .summary(&target)
        .and_then(ToyConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom);

    Ok(DirectionalConstPropResult {
        source_value,
        target_value,
        source_visits: semantics.visits(source),
        target_visits: semantics.visits(target),
    })
}

#[cfg(test)]
fn lowered_stage(pipeline: &Pipeline<Stage>) -> Result<CompileStage, ToyError> {
    pipeline.stage_by_name("lowered").ok_or_else(|| {
        ToyError::lift_from(kirin_interpreter_new::InterpreterError::Custom(
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
        .map_err(ToyError::lift_from)?;
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
    let stage = match pipeline.stage_by_name("lowered") {
        Some(stage) => stage,
        None => {
            return Err(ToyError::lift_from(
                kirin_interpreter_new::InterpreterError::Custom("missing lowered stage"),
            ));
        }
    };
    let function = resolve_function(pipeline, function_name)?;
    analyze_lowered_constprop_invocation(
        pipeline,
        FunctionInvocation::function(stage, function, Product::from_vec(args.to_vec())),
    )
}

pub fn analyze_lowered_constprop_invocation(
    pipeline: &Pipeline<Stage>,
    invocation: FunctionInvocation<ConstProp>,
) -> Result<ConstProp, ToyError> {
    let owner = ConstPropOwner::function(invocation.stage(), invocation.target());
    let args = invocation.args().iter().cloned().collect::<Vec<_>>();
    let mut interp: FunctionFixpoint<'_> =
        FunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    let mut semantics = ConstPropSemantics::new(&args);

    interp.solve(&mut semantics, owner)?;
    Ok(interp
        .summary(&owner)
        .and_then(ToyConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom))
}
