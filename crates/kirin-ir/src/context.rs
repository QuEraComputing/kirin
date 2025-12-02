use std::cell::RefCell;
use std::sync::Arc;

use crate::language::Language;
use crate::node::region::RegionInfo;
use crate::{InternTable, node::*};

#[derive(Debug)]
pub struct Context<L: Language> {
    pub(crate) staged_functions: Vec<StagedFunctionInfo<L>>,
    pub(crate) regions: Vec<RegionInfo<L>>,
    pub(crate) blocks: Vec<BlockInfo<L>>,
    pub(crate) statements: Vec<StatementInfo<L>>,
    pub(crate) ssas: Vec<SSAInfo<L>>,
    pub(crate) symbols: Arc<RefCell<InternTable<String, Symbol>>>,
}

impl<L> Default for Context<L>
where
    L: Language,
{
    fn default() -> Self {
        Self {
            staged_functions: Vec::new(),
            regions: Vec::new(),
            blocks: Vec::new(),
            statements: Vec::new(),
            ssas: Vec::new(),
            symbols: Arc::new(RefCell::new(InternTable::default())),
        }
    }
}

impl<L> Clone for Context<L>
where
    L: Language,
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

impl<L: Language> Context<L> {
    pub fn new_statement_id(&self) -> StatementId {
        StatementId(self.statements.len())
    }
}
