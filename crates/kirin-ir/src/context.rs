use std::cell::RefCell;
use std::sync::Arc;

use crate::arena::Arena;
use crate::node::region::RegionInfo;
use crate::{Dialect, InternTable, node::*};

#[derive(Debug)]
pub struct Context<L: Dialect> {
    pub(crate) staged_functions: Arena<StagedFunction, StagedFunctionInfo<L>>,
    pub(crate) regions: Arena<Region, RegionInfo<L>>,
    pub(crate) blocks: Arena<Block, BlockInfo<L>>,
    pub(crate) statements: Arena<Statement, StatementInfo<L>>,
    pub(crate) ssas: Arena<SSAValue, SSAInfo<L>>,
    pub(crate) symbols: Arc<RefCell<InternTable<String, Symbol>>>,
}

impl<L> Default for Context<L>
where
    L: Dialect,
{
    fn default() -> Self {
        Self {
            staged_functions: Arena::default(),
            regions: Arena::default(),
            blocks: Arena::default(),
            statements: Arena::default(),
            ssas: Arena::default(),
            symbols: Arc::new(RefCell::new(InternTable::default())),
        }
    }
}

impl<L> Clone for Context<L>
where
    L: Dialect,
    StatementInfo<L>: Clone,
    SSAInfo<L>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            staged_functions: self.staged_functions.clone(),
            regions: self.regions.clone(),
            blocks: self.blocks.clone(),
            statements: self.statements.clone(),
            ssas: self.ssas.clone(),
            symbols: self.symbols.clone(),
        }
    }
}

impl<L: Dialect> Context<L> {
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
    /// Read-only access. Use `borrow_mut` on the returned `RefCell` for mutable access.
    pub fn symbol_table(&self) -> Arc<RefCell<InternTable<String, Symbol>>> {
        self.symbols.clone()
    }

    /// Get a reference to the staged functions arena.
    ///
    /// Read-only access. Use `get_info_mut` on `StagedFunction` for mutable access.
    pub fn staged_function_arena(&self) -> &Arena<StagedFunction, StagedFunctionInfo<L>> {
        &self.staged_functions
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
}
