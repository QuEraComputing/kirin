use crate::node::region::RegionInfo;
use crate::node::*;
use crate::language::Language;

pub struct Arena<L: Language> {
    pub(crate) staged_functions: Vec<StagedFunctionInfo<L>>,
    pub(crate) regions: Vec<RegionInfo<L>>,
    pub(crate) blocks: Vec<BlockInfo<L>>,
    pub(crate) statements: Vec<StatementInfo<L>>,
    pub(crate) ssas: Vec<SSAInfo<L>>,
}

impl<L> Default for Arena<L>
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
        }
    }
}

impl<L> Clone for Arena<L>
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
        }
    }
}
