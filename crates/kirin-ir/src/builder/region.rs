use crate::{Block, Language, Region, StatementId, node::RegionInfo};

pub struct RegionBuilder<'a, L: Language> {
    pub(super) arena: &'a mut crate::Arena<L>,
    pub(super) parent: Option<StatementId>,
    pub(super) blocks: Vec<Block>,
}

impl<'a, L: Language> RegionBuilder<'a, L> {
    pub fn from_arena(arena: &'a mut crate::Arena<L>) -> Self {
        Self {
            arena,
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
        let id = Region(self.arena.regions.len());
        let info = RegionInfo::builder()
            .id(id)
            .blocks(self.arena.link_blocks(&self.blocks))
            .maybe_parent(self.parent)
            .new();
        self.arena.regions.push(info);
        id
    }
}
