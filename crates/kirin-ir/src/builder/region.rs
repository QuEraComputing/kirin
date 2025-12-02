use crate::{Block, Language, Region, StatementId, node::RegionInfo};

pub struct RegionBuilder<'a, L: Language> {
    pub(super) context: &'a mut crate::Context<L>,
    pub(super) parent: Option<StatementId>,
    pub(super) blocks: Vec<Block>,
}

impl<'a, L: Language> RegionBuilder<'a, L> {
    pub fn from_context(context: &'a mut crate::Context<L>) -> Self {
        Self {
            context,
            parent: None,
            blocks: Vec::new(),
        }
    }

    pub fn parent(mut self, value: Option<StatementId>) -> Self {
        self.parent = value;
        self
    }

    pub fn add_block(mut self, block: Block) -> Self {
        self.blocks.push(block);
        self
    }

    pub fn new(self) -> Region {
        let id = Region(self.context.regions.len());
        let info = RegionInfo::builder()
            .id(id)
            .blocks(self.context.link_blocks(&self.blocks))
            .maybe_parent(self.parent)
            .new();
        self.context.regions.push(info);
        id
    }
}
