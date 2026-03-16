use crate::arena::{Arena, GetInfo};
use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::function::CompileStage;
use crate::node::region::RegionInfo;
use crate::node::stmt::StatementParent;
use crate::node::ungraph::{UnGraph, UnGraphInfo};
use crate::{Dialect, InternTable, node::*};

#[derive(Debug)]
pub struct StageInfo<L: Dialect> {
    /// Optional human-readable name for this compilation stage.
    ///
    /// When set, printing infrastructure can use this instead of a numeric
    /// index (e.g., `stage @llvm_ir` instead of `stage @0`). The symbol is
    /// interned in the pipeline's global symbol table.
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
    ///
    /// Read-only access. Use `get_info_mut` on `Statement` for mutable access.
    pub fn statement_arena(&self) -> &Arena<Statement, StatementInfo<L>> {
        &self.statements
    }

    /// Get a reference to the SSA values arena.
    ///
    /// Read-only access. Use `get_info_mut` on `SSAValue` for mutable access.
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
    ///
    /// Read-only access. Use `get_info_mut` on `StagedFunction` for mutable access.
    pub fn staged_function_arena(&self) -> &Arena<StagedFunction, StagedFunctionInfo<L>> {
        &self.staged_functions
    }

    /// Get the policy controlling staged-function name/signature compatibility.
    pub fn staged_name_policy(&self) -> StagedNamePolicy {
        self.staged_name_policy
    }

    /// Set the policy controlling staged-function name/signature compatibility.
    ///
    /// Defaults to [`StagedNamePolicy::SingleInterface`].
    pub fn set_staged_name_policy(&mut self, policy: StagedNamePolicy) {
        self.staged_name_policy = policy;
    }

    /// Get a reference to the regions arena.
    ///
    /// Read-only access. Use `get_info_mut` on `Region` for mutable access.
    pub fn region_arena(&self) -> &Arena<Region, RegionInfo<L>> {
        &self.regions
    }

    /// Get a reference to the blocks arena.
    ///
    /// Read-only access. Use `get_info_mut` on `Block` for mutable access.
    pub fn block_arena(&self) -> &Arena<Block, BlockInfo<L>> {
        &self.blocks
    }

    /// Get a reference to the directed graph arena.
    ///
    /// Read-only access. Use `get_info_mut` on `DiGraph` for mutable access.
    pub fn digraph_arena(&self) -> &Arena<DiGraph, DiGraphInfo<L>> {
        &self.digraphs
    }

    /// Get a reference to the undirected graph arena.
    ///
    /// Read-only access. Use `get_info_mut` on `UnGraph` for mutable access.
    pub fn ungraph_arena(&self) -> &Arena<UnGraph, UnGraphInfo<L>> {
        &self.ungraphs
    }

    /// Attach statements and an optional terminator to an existing block.
    ///
    /// Sets each statement's parent to `block`, links the statements into a
    /// linked list, and stores them on the block info. This is used by parser
    /// emit flows that create a block (with arguments) first, then emit
    /// statements in a second phase.
    pub fn attach_statements_to_block(
        &mut self,
        block: Block,
        stmts: &[Statement],
        terminator: Option<Statement>,
    ) {
        for &stmt in stmts {
            stmt.expect_info_mut(self).parent = Some(StatementParent::Block(block));
        }
        if let Some(term) = terminator {
            term.expect_info_mut(self).parent = Some(StatementParent::Block(block));
        }
        let linked = self.link_statements(stmts);
        let block_info = block.expect_info_mut(self);
        block_info.statements = linked;
        block_info.terminator = terminator;
    }

    /// Move `real` block payload into `stub`, preserving external block IDs.
    ///
    /// This is used by parser two-pass emit flows that must pre-register block
    /// IDs for forward references, then replace stub block contents with fully
    /// emitted blocks.
    ///
    /// The remap updates all statement parents and block-argument owners from
    /// `real` to `stub`, then marks `real` deleted.
    pub fn remap_block_identity(&mut self, stub: Block, real: Block) {
        let mut real_info = real.expect_info(self).clone();
        let statements: Vec<_> = real.statements(self).collect();
        let terminator = real.terminator(self);

        for stmt in statements {
            stmt.expect_info_mut(self).parent = Some(StatementParent::Block(stub));
        }
        if let Some(term) = terminator {
            term.expect_info_mut(self).parent = Some(StatementParent::Block(stub));
        }

        for (idx, arg) in real_info.arguments.iter().copied().enumerate() {
            let arg_info = arg.expect_info_mut(self);
            if let SSAKind::BlockArgument(owner, _) = arg_info.kind {
                debug_assert_eq!(
                    owner, real,
                    "unexpected block-arg owner while remapping block identity"
                );
                arg_info.kind = SSAKind::BlockArgument(stub, idx);
            }
        }

        // Keep list-node identity coherent with the arena slot ID.
        real_info.node.ptr = stub;
        *stub.expect_info_mut(self) = real_info;
        self.blocks.delete(real);
    }
}
