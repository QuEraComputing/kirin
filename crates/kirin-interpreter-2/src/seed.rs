use kirin_ir::Block;

/// Public shell seed for entering block-local execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockSeed {
    block: Block,
}

impl BlockSeed {
    pub fn new(block: Block) -> Self {
        Self { block }
    }

    pub fn block(self) -> Block {
        self.block
    }
}

impl From<Block> for BlockSeed {
    fn from(block: Block) -> Self {
        Self::new(block)
    }
}

/// Public shell execution seeds.
///
/// MVP note: only block execution is surfaced in the first implementation
/// wave. Additional seed kinds will be added once the single-stage shell is
/// proven in code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExecutionSeed {
    Block(BlockSeed),
}

impl From<BlockSeed> for ExecutionSeed {
    fn from(seed: BlockSeed) -> Self {
        Self::Block(seed)
    }
}

impl From<Block> for ExecutionSeed {
    fn from(block: Block) -> Self {
        Self::Block(block.into())
    }
}
