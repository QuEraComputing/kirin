//! Engine-internal IR queries over stage enums.
//!
//! Engines need a handful of language-independent facts (block parameters,
//! statement order, CFG entry, function specialization) from typed
//! `StageInfo<L>` values. Each query is a [`StageAction`] dispatched through
//! kirin-ir's `StageDispatch` machinery; [`StageQuery`] bundles them into one
//! bound that any well-formed stage enum satisfies automatically.

use kirin_ir::{
    Block, Cfg, CompileStage, Dialect, GetInfo, HasArguments, HasBlocks, HasCfgs, HasStageInfo,
    HasSuccessors, Pipeline, SSAKind, SSAValue, SpecializedFunction, StageAction, StageInfo,
    StageMeta, StagedFunction, Statement, SupportsStageDispatch, Symbol,
    UniqueLiveSpecializationError,
};

use crate::Body;
use crate::InterpreterError;
use crate::facts::topology::{self, BodyTopology};

/// Block parameters as SSA values.
pub struct BlockParams(pub Block);

impl<S, L> StageAction<S, L> for BlockParams
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Vec<SSAValue>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let block = self
            .0
            .get_info(info)
            .ok_or(InterpreterError::MissingBlock(self.0))?;
        Ok(block
            .arguments
            .iter()
            .copied()
            .map(SSAValue::from)
            .collect())
    }
}

/// First statement of a block (head of the statement list, or the cached
/// terminator for terminator-only blocks).
pub struct FirstStatement(pub Block);

impl<S, L> StageAction<S, L> for FirstStatement
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Option<Statement>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(self.0.first_statement(info))
    }
}

/// Statement after `after` within `block`, ending with the terminator.
pub struct NextStatement {
    pub block: Block,
    pub after: Statement,
}

impl<S, L> StageAction<S, L> for NextStatement
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Option<Statement>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        match *self.after.next(info) {
            Some(next) => Ok(Some(next)),
            None if self.block.last_statement(info) != Some(self.after) => {
                Ok(self.block.last_statement(info))
            }
            None => Ok(None),
        }
    }
}

/// Entry block of a CFG.
pub struct CfgEntry(pub Cfg);

impl<S, L> StageAction<S, L> for CfgEntry
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Option<Block>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(self.0.blocks(info).next())
    }
}

/// Everything the default digraph walker needs: the boundary ports, the
/// node statements in topological order, and the graph's yields.
#[derive(Clone, Debug)]
pub struct GraphWalkPlan {
    pub ports: Vec<kirin_ir::Port>,
    pub schedule: Vec<Statement>,
    pub yields: Vec<SSAValue>,
}

/// The walk plan of a digraph body (ports, toposorted nodes, yields).
///
/// Fails with [`InterpreterError::GraphHasCycle`] on cyclic digraphs: they
/// are structurally legal IR but have no single-pass execution order.
pub struct DiGraphWalkQuery(pub kirin_ir::DiGraph);

impl<S, L> StageAction<S, L> for DiGraphWalkQuery
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = GraphWalkPlan;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let graph_info = self
            .0
            .get_info(info)
            .ok_or(InterpreterError::GraphHasCycle(self.0))?;
        let order = petgraph::algo::toposort(graph_info.graph(), None)
            .map_err(|_| InterpreterError::GraphHasCycle(self.0))?;
        let schedule = order
            .into_iter()
            .map(|node| graph_info.graph()[node])
            .collect();
        Ok(GraphWalkPlan {
            ports: graph_info.ports().to_vec(),
            schedule,
            yields: graph_info.yields().to_vec(),
        })
    }
}

/// The unique live specialization of a staged function.
pub struct UniqueSpecialization(pub StagedFunction);

impl<S, L> StageAction<S, L> for UniqueSpecialization
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Result<SpecializedFunction, InterpreterError>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let staged = self.0;
        let Some(staged_info) = staged.get_info(info) else {
            return Ok(Err(InterpreterError::MissingSpecialization(staged)));
        };
        Ok(match staged_info.unique_live_specialization() {
            Ok(function) => Ok(function),
            Err(UniqueLiveSpecializationError::NoSpecialization) => {
                Err(InterpreterError::MissingSpecialization(staged))
            }
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(InterpreterError::AmbiguousSpecialization {
                    function: staged,
                    count,
                })
            }
        })
    }
}

/// Body statement of a specialized function.
pub struct FunctionBody(pub SpecializedFunction);

impl<S, L> StageAction<S, L> for FunctionBody
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Result<Statement, InterpreterError>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(self
            .0
            .get_info(info)
            .map(|info| *info.body())
            .ok_or(InterpreterError::Custom("specialized function has no body")))
    }
}

/// The kind (defining site) of an SSA value: statement result, block
/// argument, or graph port.
pub struct ValueKind(pub SSAValue);

impl<S, L> StageAction<S, L> for ValueKind
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = SSAKind;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        self.0
            .get_info(info)
            .map(|value| *value.kind())
            .ok_or(InterpreterError::MissingValue(self.0))
    }
}

/// The operands of `block`'s terminator (a structured body's yield slots).
/// Empty for a terminator-less block.
pub struct TerminatorArguments(pub Block);

impl<S, L> StageAction<S, L> for TerminatorArguments
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    for<'a> L: HasArguments<'a>,
{
    type Output = Vec<SSAValue>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(self
            .0
            .terminator(info)
            .map(|terminator| {
                terminator
                    .definition(info)
                    .arguments()
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default())
    }
}

/// The topology of a body: blocks and graph parts (including nested
/// structured bodies), statements per part, CFG successors, block feeders,
/// and graph-port boundaries.
pub struct BodyTopologyQuery(pub Body);

impl<S, L> StageAction<S, L> for BodyTopologyQuery
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    for<'a> L: HasSuccessors<'a>
        + HasBlocks<'a>
        + HasCfgs<'a>
        + kirin_ir::HasDigraphs<'a>
        + kirin_ir::HasUngraphs<'a>,
{
    type Output = BodyTopology;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(topology::body_topology(info, self.0))
    }
}

/// Resolve a stage-local symbol to its interned name.
pub struct ResolveSymbolName(pub Symbol);

impl<S, L> StageAction<S, L> for ResolveSymbolName
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Option<String>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(info.symbol_table().resolve(self.0).cloned())
    }
}

/// Bound bundle for stage enums usable by interpreter engines.
///
/// Satisfied automatically by any stage enum built from `StageInfo<L>`
/// variants (and by `StageInfo<L>` itself for single-language pipelines);
/// compiler authors never implement it by hand.
pub trait StageQuery:
    StageMeta
    + SupportsStageDispatch<BlockParams, Vec<SSAValue>, InterpreterError>
    + SupportsStageDispatch<FirstStatement, Option<Statement>, InterpreterError>
    + SupportsStageDispatch<NextStatement, Option<Statement>, InterpreterError>
    + SupportsStageDispatch<CfgEntry, Option<Block>, InterpreterError>
    + SupportsStageDispatch<
        UniqueSpecialization,
        Result<SpecializedFunction, InterpreterError>,
        InterpreterError,
    > + SupportsStageDispatch<FunctionBody, Result<Statement, InterpreterError>, InterpreterError>
    + SupportsStageDispatch<ResolveSymbolName, Option<String>, InterpreterError>
    + SupportsStageDispatch<ValueKind, SSAKind, InterpreterError>
    + SupportsStageDispatch<TerminatorArguments, Vec<SSAValue>, InterpreterError>
    + SupportsStageDispatch<BodyTopologyQuery, BodyTopology, InterpreterError>
    + SupportsStageDispatch<DiGraphWalkQuery, GraphWalkPlan, InterpreterError>
{
}

impl<S> StageQuery for S where
    S: StageMeta
        + SupportsStageDispatch<BlockParams, Vec<SSAValue>, InterpreterError>
        + SupportsStageDispatch<FirstStatement, Option<Statement>, InterpreterError>
        + SupportsStageDispatch<NextStatement, Option<Statement>, InterpreterError>
        + SupportsStageDispatch<CfgEntry, Option<Block>, InterpreterError>
        + SupportsStageDispatch<
            UniqueSpecialization,
            Result<SpecializedFunction, InterpreterError>,
            InterpreterError,
        > + SupportsStageDispatch<FunctionBody, Result<Statement, InterpreterError>, InterpreterError>
        + SupportsStageDispatch<ResolveSymbolName, Option<String>, InterpreterError>
        + SupportsStageDispatch<ValueKind, SSAKind, InterpreterError>
        + SupportsStageDispatch<TerminatorArguments, Vec<SSAValue>, InterpreterError>
        + SupportsStageDispatch<BodyTopologyQuery, BodyTopology, InterpreterError>
        + SupportsStageDispatch<DiGraphWalkQuery, GraphWalkPlan, InterpreterError>
{
}

/// Run a stage action against the stage with id `stage`.
pub(crate) fn dispatch<S, A, R>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    mut action: A,
) -> Result<R, InterpreterError>
where
    S: StageMeta + SupportsStageDispatch<A, R, InterpreterError>,
{
    let info = pipeline
        .stage(stage)
        .ok_or(InterpreterError::MissingStage(stage))?;
    S::dispatch_stage_action(info, stage, &mut action)?
        .ok_or(InterpreterError::MissingStageInfo(stage))
}

pub(crate) fn block_params<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    block: Block,
) -> Result<Vec<SSAValue>, InterpreterError> {
    dispatch(pipeline, stage, BlockParams(block))
}

pub(crate) fn first_statement<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    block: Block,
) -> Result<Option<Statement>, InterpreterError> {
    dispatch(pipeline, stage, FirstStatement(block))
}

pub(crate) fn next_statement<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    block: Block,
    after: Statement,
) -> Result<Option<Statement>, InterpreterError> {
    dispatch(pipeline, stage, NextStatement { block, after })
}

pub(crate) fn cfg_entry<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    cfg: Cfg,
) -> Result<Option<Block>, InterpreterError> {
    dispatch(pipeline, stage, CfgEntry(cfg))
}

pub(crate) fn unique_specialization<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    staged: StagedFunction,
) -> Result<SpecializedFunction, InterpreterError> {
    dispatch(pipeline, stage, UniqueSpecialization(staged))?
}

pub(crate) fn function_body<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    specialized: SpecializedFunction,
) -> Result<Statement, InterpreterError> {
    dispatch(pipeline, stage, FunctionBody(specialized))?
}

pub(crate) fn resolve_symbol_name<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    symbol: Symbol,
) -> Result<Option<String>, InterpreterError> {
    dispatch(pipeline, stage, ResolveSymbolName(symbol))
}

pub(crate) fn value_kind<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    value: SSAValue,
) -> Result<SSAKind, InterpreterError> {
    dispatch(pipeline, stage, ValueKind(value))
}

pub(crate) fn terminator_arguments<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    block: Block,
) -> Result<Vec<SSAValue>, InterpreterError> {
    dispatch(pipeline, stage, TerminatorArguments(block))
}

pub(crate) fn digraph_walk_plan<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    graph: kirin_ir::DiGraph,
) -> Result<GraphWalkPlan, InterpreterError> {
    dispatch(pipeline, stage, DiGraphWalkQuery(graph))
}

pub(crate) fn body_topology<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    body: Body,
) -> Result<BodyTopology, InterpreterError> {
    dispatch(pipeline, stage, BodyTopologyQuery(body))
}
