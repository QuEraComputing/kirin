#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use kirin::prelude::TryLiftFrom;
use kirin::prelude::{
    Block, CompileStage, CompileTimeValue, Dialect, GetInfo, HasStageInfo, Lattice, LiftFrom,
    Pipeline, Product, StageMeta, Symbol, TryLift,
};
use kirin_function::interpreter_new::{CallTargetResolution, ResolvedCallTarget};
use kirin_interpreter_new::{
    AbstractBlockTransfer, AbstractEnvStore, BlockFrame, BlockTransfer, Env, FunctionEntryTarget,
    FunctionInvocation, FunctionInvocationDispatch, InterpreterError, Location, OwnerSemantics,
    OwnerSummaryDeps, StageAccess, StandardFixpointInterpreter, Summary, SummaryEffect,
};
#[cfg(test)]
use kirin_interpreter_new::{
    BackwardSummaryDeps, ForwardSummaryDeps, FunctionFrame, SpecializedFunctionFrame,
    StagedFunctionFrame, SummaryDependency, SummaryDependencyIndex,
};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

use kirin_scf::ForLoopValue;
use kirin_scf::interpreter_new::{ForContinuation, ScfCompletion, ScfForFixpointSummary};

#[cfg(test)]
use super::ToyFrame;
use super::run::expect_function_return;
use super::run::resolve_function;
use super::{ConstProp, ToyCompletion, ToyError, ToyStageFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FunctionOwner {
    stage: CompileStage,
    target: FunctionEntryTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ConstPropOwner {
    Function(FunctionOwner),
    ScfFor { location: Location },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScfForSummary {
    body: Block,
    init_arg_count: usize,
    iv: ConstProp,
    end: ConstProp,
    step: ConstProp,
    carried: Product<ConstProp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConstPropSummary {
    Function { value: ConstProp },
    ScfFor { state: Option<ScfForSummary> },
}

impl ConstPropSummary {
    fn function_bottom() -> Self {
        Self::Function {
            value: ConstProp::Bottom,
        }
    }

    fn scf_for_bottom() -> Self {
        Self::ScfFor { state: None }
    }

    fn function_value(&self) -> Option<&ConstProp> {
        match self {
            Self::Function { value } => Some(value),
            Self::ScfFor { .. } => None,
        }
    }

    fn scf_for_state(&self) -> Option<&ScfForSummary> {
        match self {
            Self::ScfFor { state } => state.as_ref(),
            Self::Function { .. } => None,
        }
    }
}

impl Summary for ConstPropSummary {
    type Strategy = ();
    type Change = ();

    fn merge(
        &mut self,
        _phase: kirin_interpreter_new::FixpointPhase,
        candidate: Self,
        _strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        match (self, candidate) {
            (Self::Function { value }, Self::Function { value: candidate }) => {
                let joined = value.join(&candidate);
                if *value == joined {
                    None
                } else {
                    *value = joined;
                    Some(())
                }
            }
            (
                Self::ScfFor { state },
                Self::ScfFor {
                    state: Some(candidate),
                },
            ) => merge_scf_for_summary(state, candidate),
            (Self::ScfFor { .. }, Self::ScfFor { state: None }) => None,
            _ => None,
        }
    }
}

impl<L, T, X> ScfForFixpointSummary<L, T, ConstProp, X, ConstPropOwner> for ConstPropSummary
where
    L: Dialect,
    T: CompileTimeValue,
    X: BlockTransfer<Value = ConstProp>,
{
    fn scf_for_owner(continuation: &ForContinuation<L, T, ConstProp, X>) -> Option<ConstPropOwner> {
        Some(ConstPropOwner::ScfFor {
            location: continuation.location,
        })
    }

    fn scf_for_initial_summary(continuation: &ForContinuation<L, T, ConstProp, X>) -> Self {
        Self::ScfFor {
            state: Some(ScfForSummary {
                body: continuation.body,
                init_arg_count: continuation.init_args.len(),
                iv: continuation.iv.clone(),
                end: continuation.end.clone(),
                step: continuation.step.clone(),
                carried: continuation.carried.clone(),
            }),
        }
    }

    fn scf_for_results(&self) -> Option<Product<ConstProp>> {
        self.scf_for_state().map(|state| state.carried.clone())
    }
}

fn merge_scf_for_summary(
    state: &mut Option<ScfForSummary>,
    candidate: ScfForSummary,
) -> Option<()> {
    let Some(current) = state else {
        *state = Some(candidate);
        return Some(());
    };

    let joined_iv = current.iv.join(&candidate.iv);
    let joined_end = current.end.join(&candidate.end);
    let joined_step = current.step.join(&candidate.step);
    let joined_carried = current
        .carried
        .iter()
        .zip(candidate.carried.iter())
        .map(|(current, candidate)| current.join(candidate))
        .collect::<Product<_>>();
    if current.iv == joined_iv
        && current.end == joined_end
        && current.step == joined_step
        && current.carried == joined_carried
    {
        None
    } else {
        current.iv = joined_iv;
        current.end = joined_end;
        current.step = joined_step;
        current.carried = joined_carried;
        Some(())
    }
}

fn scf_for_body_args(state: &ScfForSummary) -> Product<ConstProp> {
    let mut args = Vec::with_capacity(1 + state.init_arg_count);
    args.push(state.iv.clone());
    args.extend(state.carried.iter().take(state.init_arg_count).cloned());
    Product::from_vec(args)
}

fn owner_stage(owner: &ConstPropOwner) -> CompileStage {
    match owner {
        ConstPropOwner::Function(owner) => owner.stage,
        ConstPropOwner::ScfFor { location } => location.stage,
    }
}

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

type FunctionFixpoint<'ir> = StandardFixpointInterpreter<
    'ir,
    Stage,
    ConstPropOwner,
    ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>,
    ToyCompletion<ConstProp>,
    ToyError,
    ConstPropSummary,
    AbstractEnvStore<ConstProp>,
    OwnerSummaryDeps<ConstPropOwner>,
>;

#[cfg(test)]
type DirectionalFunctionFixpoint<'ir, Deps> = StandardFixpointInterpreter<
    'ir,
    Stage,
    ConstPropOwner,
    ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
    ToyCompletion<ConstProp>,
    ToyError,
    ConstPropSummary,
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
        ConstPropSummary,
        ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
    > for ConstPropSemantics
{
    fn bottom_summary(
        &mut self,
        _interp: &mut FunctionFixpoint<'ir>,
        owner: &ConstPropOwner,
    ) -> Result<ConstPropSummary, ToyError> {
        Ok(match owner {
            ConstPropOwner::Function(_) => ConstPropSummary::function_bottom(),
            ConstPropOwner::ScfFor { .. } => ConstPropSummary::scf_for_bottom(),
        })
    }

    fn entry_frame(
        &mut self,
        interp: &mut FunctionFixpoint<'ir>,
        owner: &ConstPropOwner,
        summary: &ConstPropSummary,
    ) -> Result<ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>, ToyError> {
        match owner {
            ConstPropOwner::Function(owner) => {
                interp.dispatch_function_invocation(FunctionInvocation::new(
                    owner.stage,
                    owner.target,
                    Product::from_vec(self.args.clone()),
                ))
            }
            ConstPropOwner::ScfFor { .. } => {
                let state = summary.scf_for_state().ok_or_else(|| {
                    ToyError::lift_from(InterpreterError::Custom("missing scf.for summary state"))
                })?;
                let env = interp.alloc();
                match interp.pipeline().stage(owner_stage(owner)) {
                    Some(Stage::Source(_)) => {
                        BlockFrame::<HighLevel, ConstProp, AbstractBlockTransfer<ConstProp>>::new(
                            owner_stage(owner),
                            state.body,
                            env,
                            scf_for_body_args(state),
                        )
                        .try_lift()
                        .map_err(ToyError::from)
                    }
                    Some(Stage::Lowered(_)) => Err(ToyError::lift_from(InterpreterError::Custom(
                        "scf.for fixpoint owner cannot run at lowered stage",
                    ))),
                    None => Err(ToyError::lift_from(InterpreterError::MissingStage(
                        owner_stage(owner),
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
    ) -> Result<SummaryEffect<ConstPropOwner, ConstPropSummary>, ToyError> {
        match owner {
            ConstPropOwner::Function(_) => Ok(SummaryEffect::Update {
                owner,
                candidate: ConstPropSummary::Function {
                    value: expect_function_return(completion)?,
                },
            }),
            ConstPropOwner::ScfFor { .. } => {
                let current = interp
                    .summary(&owner)
                    .and_then(ConstPropSummary::scf_for_state)
                    .cloned()
                    .ok_or_else(|| {
                        ToyError::lift_from(InterpreterError::Custom(
                            "missing scf.for summary during completion",
                        ))
                    })?;
                let carried = expect_scf_yield(completion)?;
                let next_iv = current
                    .iv
                    .loop_step(&current.step)
                    .ok_or_else(|| ToyError::lift_from(InterpreterError::LoopStepOverflow))?;
                Ok(SummaryEffect::Update {
                    owner,
                    candidate: ConstPropSummary::ScfFor {
                        state: Some(ScfForSummary {
                            iv: next_iv,
                            carried,
                            ..current
                        }),
                    },
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
        ConstPropSummary,
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
    ) -> Result<ConstPropSummary, ToyError> {
        Ok(match owner {
            ConstPropOwner::Function(_) => ConstPropSummary::function_bottom(),
            ConstPropOwner::ScfFor { .. } => ConstPropSummary::scf_for_bottom(),
        })
    }

    fn entry_frame(
        &mut self,
        _interp: &mut DirectionalFunctionFixpoint<'ir, Deps>,
        owner: &ConstPropOwner,
        _summary: &ConstPropSummary,
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
    ) -> Result<SummaryEffect<ConstPropOwner, ConstPropSummary>, ToyError> {
        Ok(SummaryEffect::Update {
            owner,
            candidate: ConstPropSummary::Function {
                value: expect_function_return(completion)?,
            },
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
    let owner = ConstPropOwner::Function(FunctionOwner {
        stage: invocation.stage(),
        target: invocation.target(),
    });
    let args = invocation.args().iter().cloned().collect::<Vec<_>>();
    let mut interp: FunctionFixpoint<'_> =
        StandardFixpointInterpreter::new(pipeline, AbstractEnvStore::new(), ());
    let mut semantics = ConstPropSemantics::new(&args);

    interp.solve(&mut semantics, owner)?;
    Ok(interp
        .summary(&owner)
        .and_then(ConstPropSummary::function_value)
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
        .and_then(ConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom);
    let target_value = interp
        .summary(&target)
        .and_then(ConstPropSummary::function_value)
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
) -> Result<FunctionOwner, ToyError> {
    let function = pipeline
        .resolve_staged_function(function_name, stage)
        .ok_or(InterpreterError::Custom("missing staged function"))
        .map_err(ToyError::lift_from)?;
    Ok(FunctionOwner {
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
    let owner = ConstPropOwner::Function(FunctionOwner {
        stage: invocation.stage(),
        target: invocation.target(),
    });
    let args = invocation.args().iter().cloned().collect::<Vec<_>>();
    let mut interp: FunctionFixpoint<'_> =
        StandardFixpointInterpreter::new(pipeline, AbstractEnvStore::new(), ());
    let mut semantics = ConstPropSemantics::new(&args);

    interp.solve(&mut semantics, owner)?;
    Ok(interp
        .summary(&owner)
        .and_then(ConstPropSummary::function_value)
        .cloned()
        .unwrap_or(ConstProp::Bottom))
}
