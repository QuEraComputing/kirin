//! Engine-internal IR queries over stage enums.
//!
//! Engines need a handful of language-independent facts (block parameters,
//! statement order, region entry, function specialization) from typed
//! `StageInfo<L>` values. Each query is a [`StageAction`] dispatched through
//! kirin-ir's `StageDispatch` machinery; [`StageQuery`] bundles them into one
//! bound that any well-formed stage enum satisfies automatically.

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, Region, SSAValue,
    SpecializedFunction, StageAction, StageInfo, StageMeta, StagedFunction, Statement,
    SupportsStageDispatch, Symbol, UniqueLiveSpecializationError,
};

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

/// Entry block of a region.
pub struct RegionEntry(pub Region);

impl<S, L> StageAction<S, L> for RegionEntry
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
    + SupportsStageDispatch<RegionEntry, Option<Block>, InterpreterError>
    + SupportsStageDispatch<
        UniqueSpecialization,
        Result<SpecializedFunction, InterpreterError>,
        InterpreterError,
    > + SupportsStageDispatch<FunctionBody, Result<Statement, InterpreterError>, InterpreterError>
    + SupportsStageDispatch<ResolveSymbolName, Option<String>, InterpreterError>
{
}

impl<S> StageQuery for S where
    S: StageMeta
        + SupportsStageDispatch<BlockParams, Vec<SSAValue>, InterpreterError>
        + SupportsStageDispatch<FirstStatement, Option<Statement>, InterpreterError>
        + SupportsStageDispatch<NextStatement, Option<Statement>, InterpreterError>
        + SupportsStageDispatch<RegionEntry, Option<Block>, InterpreterError>
        + SupportsStageDispatch<
            UniqueSpecialization,
            Result<SpecializedFunction, InterpreterError>,
            InterpreterError,
        > + SupportsStageDispatch<FunctionBody, Result<Statement, InterpreterError>, InterpreterError>
        + SupportsStageDispatch<ResolveSymbolName, Option<String>, InterpreterError>
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

pub(crate) fn region_entry<S: StageQuery>(
    pipeline: &Pipeline<S>,
    stage: CompileStage,
    region: Region,
) -> Result<Option<Block>, InterpreterError> {
    dispatch(pipeline, stage, RegionEntry(region))
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
