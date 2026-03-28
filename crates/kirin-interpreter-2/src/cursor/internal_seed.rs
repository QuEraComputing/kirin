use kirin_ir::{Block, DiGraph, Region, Statement, UnGraph};

/// Internal cursor position within a block.
///
/// This is the old `BlockStart` — it tracks where within a block the cursor
/// should resume after an invoke or control transfer. Not part of the public
/// effect system; only the cursor/activation machinery uses this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BlockStart {
    Entry,
    Statement(Statement),
    Exhausted,
}

/// Internal block seed carrying cursor position (block + start point).
///
/// Used by the activation/continuation system to resume execution at
/// a specific point within a block. The public `BlockSeed<V>` always
/// enters at block entry; this type handles mid-block resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InternalBlockSeed {
    block: Block,
    start: BlockStart,
}

impl InternalBlockSeed {
    pub(crate) fn new(block: Block) -> Self {
        Self {
            block,
            start: BlockStart::Entry,
        }
    }

    pub(crate) fn at_statement(block: Block, statement: Statement) -> Self {
        Self {
            block,
            start: BlockStart::Statement(statement),
        }
    }

    pub(crate) fn exhausted(block: Block) -> Self {
        Self {
            block,
            start: BlockStart::Exhausted,
        }
    }

    pub(crate) fn block(self) -> Block {
        self.block
    }

    pub(crate) fn start(self) -> Option<Statement> {
        match self.start {
            BlockStart::Entry => None,
            BlockStart::Statement(statement) => Some(statement),
            BlockStart::Exhausted => None,
        }
    }

    pub(crate) fn starts_at_entry(self) -> bool {
        matches!(self.start, BlockStart::Entry)
    }

    pub(crate) fn is_exhausted(self) -> bool {
        matches!(self.start, BlockStart::Exhausted)
    }
}

impl From<Block> for InternalBlockSeed {
    fn from(block: Block) -> Self {
        Self::new(block)
    }
}

/// Internal execution seed enum for the cursor system.
///
/// Represents all the different body shapes the cursor can walk.
/// This is the old `ExecutionSeed` enum, now internal to the cursor system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InternalSeed {
    Block(InternalBlockSeed),
    Region(Region),
    DiGraph(DiGraph),
    UnGraph(UnGraph),
}

impl From<InternalBlockSeed> for InternalSeed {
    fn from(seed: InternalBlockSeed) -> Self {
        Self::Block(seed)
    }
}

impl From<Block> for InternalSeed {
    fn from(block: Block) -> Self {
        Self::Block(block.into())
    }
}

impl From<Region> for InternalSeed {
    fn from(region: Region) -> Self {
        Self::Region(region)
    }
}

impl From<DiGraph> for InternalSeed {
    fn from(digraph: DiGraph) -> Self {
        Self::DiGraph(digraph)
    }
}

impl From<UnGraph> for InternalSeed {
    fn from(ungraph: UnGraph) -> Self {
        Self::UnGraph(ungraph)
    }
}
