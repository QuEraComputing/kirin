use crate::{Block, Dialect, Region, Statement, node::RegionInfo};

pub struct RegionBuilder<'a, L: Dialect> {
    pub(super) stage: &'a mut crate::StageInfo<L>,
    pub(super) parent: Option<Statement>,
    pub(super) blocks: Vec<Block>,
}

impl<'a, L: Dialect> RegionBuilder<'a, L> {
    pub fn from_stage(stage: &'a mut crate::StageInfo<L>) -> Self {
        Self {
            stage,
            parent: None,
            blocks: Vec::new(),
        }
    }

    pub fn parent(mut self, value: Option<Statement>) -> Self {
        self.parent = value;
        self
    }

    pub fn add_block(mut self, block: Block) -> Self {
        // check if block is already pushed?
        if self.blocks.contains(&block) {
            panic!("Block `{}` is already added to the region", block);
        }
        self.blocks.push(block);
        self
    }

    pub fn new(self) -> Region {
        let id = self.stage.regions.next_id();
        let info = RegionInfo::builder()
            .id(id)
            .blocks(self.stage.link_blocks(&self.blocks))
            .maybe_parent(self.parent)
            .new();
        self.stage.regions.alloc(info);
        id
    }
}
