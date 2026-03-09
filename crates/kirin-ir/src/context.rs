use crate::arena::{Arena, GetInfo};
use crate::node::function::CompileStage;
use crate::node::region::RegionInfo;
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

    /// Get a mutable reference to the SSA values arena.
    pub fn ssa_arena_mut(&mut self) -> &mut Arena<SSAValue, SSAInfo<L>> {
        &mut self.ssas
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

    /// Get a mutable reference to the blocks arena.
    pub fn block_arena_mut(&mut self) -> &mut Arena<Block, BlockInfo<L>> {
        &mut self.blocks
    }

    /// Pre-allocate a block with typed arguments, returning the block ID and
    /// real `BlockArgument` SSAs. Statements can then reference these SSAs
    /// during emit, and the block can be finalized later via
    /// [`finalize_block`](Self::finalize_block).
    ///
    /// This avoids `BuilderBlockArgument` placeholders, which inner (nested)
    /// block builders may incorrectly try to resolve.
    pub fn pre_allocate_block(
        &mut self,
        name: impl Into<String>,
        arg_types: Vec<(String, L::Type)>,
    ) -> (Block, Vec<BlockArgument>) {
        let block_id = self.blocks.next_id();
        let name_sym = self.symbols.intern(name.into());

        // Allocate a stub block.
        let stub = BlockInfo::builder()
            .name(name_sym)
            .node(LinkedListNode::new(block_id))
            .arguments(Vec::new())
            .new();
        self.blocks.alloc(stub);

        // Create real BlockArgument SSAs.
        let mut args = Vec::with_capacity(arg_types.len());
        for (idx, (arg_name, ty)) in arg_types.into_iter().enumerate() {
            let arg: BlockArgument = self.ssas.next_id().into();
            let ssa = SSAInfo::new(
                arg.into(),
                Some(self.symbols.intern(arg_name)),
                ty,
                SSAKind::BlockArgument(block_id, idx),
            );
            self.ssas.alloc(ssa);
            args.push(arg);
        }

        (block_id, args)
    }

    /// Finalize a pre-allocated block by attaching statements and a terminator.
    ///
    /// Must be called after [`pre_allocate_block`](Self::pre_allocate_block).
    /// Statements' parent fields are updated to point to this block.
    pub fn finalize_block(
        &mut self,
        block_id: Block,
        block_args: Vec<BlockArgument>,
        stmt_ids: Vec<Statement>,
        terminator: Option<Statement>,
    ) {
        // Set parent on all statements.
        for &stmt in &stmt_ids {
            self.statements[stmt].parent = Some(block_id);
        }
        if let Some(term) = terminator {
            self.statements[term].parent = Some(block_id);
        }

        let linked = self.link_statements(&stmt_ids);

        let block_info = self
            .blocks
            .get_mut(block_id)
            .expect("pre-allocated block should exist");
        block_info.arguments = block_args;
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
            stmt.expect_info_mut(self).parent = Some(stub);
        }
        if let Some(term) = terminator {
            term.expect_info_mut(self).parent = Some(stub);
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
