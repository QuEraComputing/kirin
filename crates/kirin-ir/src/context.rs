use crate::arena::Arena;
use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::function::CompileStage;
use crate::node::region::RegionInfo;
use crate::node::ssa::SSAInfo;
use crate::node::ungraph::{UnGraph, UnGraphInfo};
use crate::{BuilderStageInfo, Dialect, InternTable, node::*};

/// The core stage info type for finalized IR.
///
/// After [`BuilderStageInfo::finalize`](crate::BuilderStageInfo::finalize),
/// every SSA value is guaranteed to have a type and a resolved kind.
/// The SSA arena holds clean [`SSAInfo`] values with `L::Type` and [`SSAKind`].
#[derive(Debug)]
pub struct StageInfo<L: Dialect> {
    /// Optional human-readable name for this compilation stage.
    pub(crate) name: Option<GlobalSymbol>,
    pub(crate) stage_id: Option<CompileStage>,
    pub(crate) staged_functions: Arena<StagedFunction, StagedFunctionInfo<L>>,
    pub(crate) staged_name_policy: StagedNamePolicy,
    pub(crate) regions: Arena<Region, RegionInfo<L>>,
    pub(crate) blocks: Arena<Block, BlockInfo<L>>,
    pub(crate) statements: Arena<Statement, StatementInfo<L>>,
    pub(crate) ssas: Arena<SSAValue, SSAInfo<L>>,
    pub(crate) digraphs: Arena<DiGraph, DiGraphInfo<L>>,
    pub(crate) ungraphs: Arena<UnGraph, UnGraphInfo<L>>,
    pub(crate) symbols: InternTable<String, Symbol>,
}

impl<L> Default for StageInfo<L>
where
    L: Dialect,
{
    fn default() -> Self {
        Self {
            name: None,
            stage_id: None,
            staged_functions: Arena::default(),
            staged_name_policy: StagedNamePolicy::default(),
            regions: Arena::default(),
            blocks: Arena::default(),
            statements: Arena::default(),
            ssas: Arena::default(),
            digraphs: Arena::default(),
            ungraphs: Arena::default(),
            symbols: InternTable::default(),
        }
    }
}

impl<L> Clone for StageInfo<L>
where
    L: Dialect,
    StatementInfo<L>: Clone,
    SSAInfo<L>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            stage_id: self.stage_id,
            staged_functions: self.staged_functions.clone(),
            staged_name_policy: self.staged_name_policy,
            regions: self.regions.clone(),
            blocks: self.blocks.clone(),
            statements: self.statements.clone(),
            ssas: self.ssas.clone(),
            digraphs: self.digraphs.clone(),
            ungraphs: self.ungraphs.clone(),
            symbols: self.symbols.clone(),
        }
    }
}

impl<L: Dialect> StageInfo<L> {
    /// Get the optional stage name for this context.
    pub fn name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    /// Set the stage name for this context.
    pub fn set_name(&mut self, name: Option<GlobalSymbol>) {
        self.name = name;
    }

    /// Get the compile-stage ID assigned by the pipeline, if any.
    pub fn stage_id(&self) -> Option<CompileStage> {
        self.stage_id
    }

    /// Set the compile-stage ID for this context.
    pub fn set_stage_id(&mut self, id: Option<CompileStage>) {
        self.stage_id = id;
    }

    /// Get a reference to the statements arena.
    pub fn statement_arena(&self) -> &Arena<Statement, StatementInfo<L>> {
        &self.statements
    }

    /// Get a reference to the SSA values arena.
    pub fn ssa_arena(&self) -> &Arena<SSAValue, SSAInfo<L>> {
        &self.ssas
    }

    /// Get a reference to the symbols intern table.
    pub fn symbol_table(&self) -> &InternTable<String, Symbol> {
        &self.symbols
    }

    /// Get a mutable reference to the symbols intern table.
    pub fn symbol_table_mut(&mut self) -> &mut InternTable<String, Symbol> {
        &mut self.symbols
    }

    /// Get a reference to the staged functions arena.
    pub fn staged_function_arena(&self) -> &Arena<StagedFunction, StagedFunctionInfo<L>> {
        &self.staged_functions
    }

    /// Get the policy controlling staged-function name/signature compatibility.
    pub fn staged_name_policy(&self) -> StagedNamePolicy {
        self.staged_name_policy
    }

    /// Set the policy controlling staged-function name/signature compatibility.
    pub fn set_staged_name_policy(&mut self, policy: StagedNamePolicy) {
        self.staged_name_policy = policy;
    }

    /// Get a reference to the regions arena.
    pub fn region_arena(&self) -> &Arena<Region, RegionInfo<L>> {
        &self.regions
    }

    /// Get a reference to the blocks arena.
    pub fn block_arena(&self) -> &Arena<Block, BlockInfo<L>> {
        &self.blocks
    }

    /// Get a reference to the directed graph arena.
    pub fn digraph_arena(&self) -> &Arena<DiGraph, DiGraphInfo<L>> {
        &self.digraphs
    }

    /// Get a reference to the undirected graph arena.
    pub fn ungraph_arena(&self) -> &Arena<UnGraph, UnGraphInfo<L>> {
        &self.ungraphs
    }

    /// Temporarily convert this `StageInfo` into a [`BuilderStageInfo`], run
    /// the closure, then convert back.
    ///
    /// This is the bridge for code paths (e.g. [`Pipeline`](crate::Pipeline))
    /// that hold `&mut StageInfo<L>` via trait dispatch but need builder methods.
    pub fn with_builder<R>(&mut self, f: impl FnOnce(&mut BuilderStageInfo<L>) -> R) -> R {
        let stage = std::mem::take(self);
        let mut builder = BuilderStageInfo::from(stage);
        let result = f(&mut builder);
        *self = builder.into_inner();
        result
    }
}
