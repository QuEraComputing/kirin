use crate::{Block, BuilderStageInfo, CFG, Dialect, Statement, node::CFGInfo};

pub struct CFGBuilder<'a, L: Dialect> {
    pub(super) stage: &'a mut BuilderStageInfo<L>,
    pub(super) parent: Option<Statement>,
    pub(super) blocks: Vec<Block>,
}

impl<'a, L: Dialect> CFGBuilder<'a, L> {
    pub fn from_stage(stage: &'a mut BuilderStageInfo<L>) -> Self {
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
        if self.blocks.contains(&block) {
            panic!("Block `{}` is already added to the cfg", block);
        }
        self.blocks.push(block);
        self
    }

    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> CFG {
        let id = self.stage.cfgs.next_id();
        let info = CFGInfo::builder()
            .id(id)
            .blocks(self.stage.link_blocks(&self.blocks))
            .maybe_parent(self.parent)
            .new();
        let _ = self.stage.cfgs.alloc(info);
        id
    }
}
