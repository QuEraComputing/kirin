//! Engine-internal IR queries over stage enums.
//!
//! Engines need a handful of language-independent facts (block parameters,
//! statement order, CFG entry, function specialization) from typed
//! `StageInfo<L>` values. Each query is a [`StageAction`] dispatched through
//! kirin-ir's `StageDispatch` machinery; [`StageQuery`] bundles them into one
//! bound that any well-formed stage enum satisfies automatically.

use kirin_ir::{
    Block, BlockParent, CFG, CompileStage, Dialect, GetInfo, HasArguments, HasBlocks, HasCFG,
    HasDigraphs, HasStageInfo, HasUngraphs, Pipeline, PortParent, SSAKind, SSAValue,
    SpecializedFunction, StageAction, StageInfo, StageMeta, StagedFunction, Statement,
    SupportsStageDispatch, Symbol, UniqueLiveSpecializationError,
};

use crate::Body;
use crate::InterpreterError;

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

/// A block's statements in program order, with the terminator last.
pub struct BlockStatements(pub Block);

impl BlockStatements {
    fn collect<L: Dialect>(&self, info: &StageInfo<L>) -> Result<Vec<Statement>, InterpreterError> {
        self.0
            .get_info(info)
            .ok_or(InterpreterError::MissingBlock(self.0))?;
        let mut statements: Vec<Statement> = self.0.statements(info).collect();
        if let Some(terminator) = self.0.terminator(info) {
            statements.push(terminator);
        }
        Ok(statements)
    }
}

impl<S, L> StageAction<S, L> for BlockStatements
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Vec<Statement>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        self.collect(info)
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
pub struct CFGEntry(pub CFG);

impl<S, L> StageAction<S, L> for CFGEntry
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

/// Statements whose backward rules translate demand on a block argument.
///
/// A directly owned single-block body is translated by its structural owner.
/// A CFG block is translated by the terminator of each block in its finalized
/// predecessor index.
pub struct BlockArgumentPredecessors(pub Block);

impl<S, L> StageAction<S, L> for BlockArgumentPredecessors
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Vec<Statement>;
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

        match block.parent {
            Some(BlockParent::Statement(owner)) => Ok(vec![owner]),
            Some(BlockParent::CFG(_)) => block
                .predecessors
                .iter()
                .map(|predecessor| {
                    predecessor.terminator(info).ok_or(InterpreterError::Custom(
                        "CFG predecessor block has no terminator",
                    ))
                })
                .collect(),
            None => Ok(Vec::new()),
        }
    }
}

/// The statement structurally owning a graph port's parent graph.
///
/// The port's [`PortParent`] identifies the authoritative graph, whose
/// `GraphInfo::parent` field identifies the statement whose dialect rule
/// translates values and demand across the graph boundary.
pub struct GraphPortOwner(pub PortParent);

impl<S, L> StageAction<S, L> for GraphPortOwner
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Statement;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let owner = match self.0 {
            PortParent::DiGraph(graph) => graph.get_info(info).and_then(|graph| graph.parent()),
            PortParent::UnGraph(graph) => graph.get_info(info).and_then(|graph| graph.parent()),
        };
        owner.ok_or(InterpreterError::Custom(
            "graph port has no owning statement",
        ))
    }
}

/// Blocks directly selected as dense fixpoint owners by an analysis root.
pub struct DirectBodyBlocks(pub Body);

impl<S, L> StageAction<S, L> for DirectBodyBlocks
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output = Vec<Block>;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(match self.0 {
            Body::CFG(cfg) => cfg.blocks(info).collect(),
            Body::Block(block) => vec![block],
            Body::DiGraph(_) | Body::UnGraph(_) => Vec::new(),
        })
    }
}

/// One step of a body-containment walk: the statements directly in this body
/// part and the child body parts reached from it.
pub struct BodyContents {
    pub statements: Vec<Statement>,
    pub children: Vec<Body>,
}

pub struct BodyContentsQuery(pub Body);

impl<S, L> StageAction<S, L> for BodyContentsQuery
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    for<'a> L: HasBlocks<'a> + HasCFG<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    type Output = BodyContents;
    type Error = InterpreterError;

    fn run(
        &mut self,
        _stage: CompileStage,
        info: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        let (statements, mut children) = match self.0 {
            Body::CFG(cfg) => (
                Vec::new(),
                cfg.blocks(info).map(Body::Block).collect::<Vec<_>>(),
            ),
            Body::Block(block) => (BlockStatements(block).collect(info)?, Vec::new()),
            Body::DiGraph(graph) => (
                graph
                    .expect_info(info)
                    .graph()
                    .node_weights()
                    .copied()
                    .collect(),
                Vec::new(),
            ),
            Body::UnGraph(graph) => (
                graph
                    .expect_info(info)
                    .graph()
                    .node_weights()
                    .copied()
                    .collect(),
                Vec::new(),
            ),
        };

        for &statement in &statements {
            let definition = statement.definition(info);
            children.extend(definition.blocks().copied().map(Body::Block));
            children.extend(definition.cfgs().copied().map(Body::CFG));
            children.extend(definition.digraphs().copied().map(Body::DiGraph));
            children.extend(definition.ungraphs().copied().map(Body::UnGraph));
        }

        Ok(BodyContents {
            statements,
            children,
        })
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
    + SupportsStageDispatch<BlockStatements, Vec<Statement>, InterpreterError>
    + SupportsStageDispatch<FirstStatement, Option<Statement>, InterpreterError>
    + SupportsStageDispatch<NextStatement, Option<Statement>, InterpreterError>
    + SupportsStageDispatch<CFGEntry, Option<Block>, InterpreterError>
    + SupportsStageDispatch<
        UniqueSpecialization,
        Result<SpecializedFunction, InterpreterError>,
        InterpreterError,
    > + SupportsStageDispatch<FunctionBody, Result<Statement, InterpreterError>, InterpreterError>
    + SupportsStageDispatch<ResolveSymbolName, Option<String>, InterpreterError>
    + SupportsStageDispatch<ValueKind, SSAKind, InterpreterError>
    + SupportsStageDispatch<TerminatorArguments, Vec<SSAValue>, InterpreterError>
    + SupportsStageDispatch<BlockArgumentPredecessors, Vec<Statement>, InterpreterError>
    + SupportsStageDispatch<GraphPortOwner, Statement, InterpreterError>
    + SupportsStageDispatch<DirectBodyBlocks, Vec<Block>, InterpreterError>
    + SupportsStageDispatch<BodyContentsQuery, BodyContents, InterpreterError>
    + SupportsStageDispatch<DiGraphWalkQuery, GraphWalkPlan, InterpreterError>
{
}

impl<S> StageQuery for S where
    S: StageMeta
        + SupportsStageDispatch<BlockParams, Vec<SSAValue>, InterpreterError>
        + SupportsStageDispatch<BlockStatements, Vec<Statement>, InterpreterError>
        + SupportsStageDispatch<FirstStatement, Option<Statement>, InterpreterError>
        + SupportsStageDispatch<NextStatement, Option<Statement>, InterpreterError>
        + SupportsStageDispatch<CFGEntry, Option<Block>, InterpreterError>
        + SupportsStageDispatch<
            UniqueSpecialization,
            Result<SpecializedFunction, InterpreterError>,
            InterpreterError,
        > + SupportsStageDispatch<FunctionBody, Result<Statement, InterpreterError>, InterpreterError>
        + SupportsStageDispatch<ResolveSymbolName, Option<String>, InterpreterError>
        + SupportsStageDispatch<ValueKind, SSAKind, InterpreterError>
        + SupportsStageDispatch<TerminatorArguments, Vec<SSAValue>, InterpreterError>
        + SupportsStageDispatch<BlockArgumentPredecessors, Vec<Statement>, InterpreterError>
        + SupportsStageDispatch<GraphPortOwner, Statement, InterpreterError>
        + SupportsStageDispatch<DirectBodyBlocks, Vec<Block>, InterpreterError>
        + SupportsStageDispatch<BodyContentsQuery, BodyContents, InterpreterError>
        + SupportsStageDispatch<DiGraphWalkQuery, GraphWalkPlan, InterpreterError>
{
}
/// TODO: add caching (with red-green tree) to avoid repeated queries for the same stage and block/statement.
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

pub(crate) fn block_statements<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    block: Block,
) -> Result<Vec<Statement>, InterpreterError> {
    dispatch(pipeline, stage, BlockStatements(block))
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
    cfg: CFG,
) -> Result<Option<Block>, InterpreterError> {
    dispatch(pipeline, stage, CFGEntry(cfg))
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

pub(crate) fn block_argument_predecessors<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    block: Block,
) -> Result<Vec<Statement>, InterpreterError> {
    dispatch(pipeline, stage, BlockArgumentPredecessors(block))
}

pub(crate) fn graph_port_owner<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    parent: PortParent,
) -> Result<Statement, InterpreterError> {
    dispatch(pipeline, stage, GraphPortOwner(parent))
}

pub(crate) fn digraph_walk_plan<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    graph: kirin_ir::DiGraph,
) -> Result<GraphWalkPlan, InterpreterError> {
    dispatch(pipeline, stage, DiGraphWalkQuery(graph))
}

pub(crate) fn direct_body_blocks<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    body: Body,
) -> Result<Vec<Block>, InterpreterError> {
    dispatch(pipeline, stage, DirectBodyBlocks(body))
}

pub(crate) fn body_contents<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    body: Body,
) -> Result<BodyContents, InterpreterError> {
    dispatch(pipeline, stage, BodyContentsQuery(body))
}
