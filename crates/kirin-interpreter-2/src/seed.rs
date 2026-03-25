use kirin_ir::{Block, DiGraph, Region, Statement, UnGraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BlockStart {
    Entry,
    Statement(Statement),
    Exhausted,
}

/// Public shell seed for entering block-local execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockSeed {
    block: Block,
    start: BlockStart,
}

impl BlockSeed {
    pub fn new(block: Block) -> Self {
        Self {
            block,
            start: BlockStart::Entry,
        }
    }

    pub fn at_statement(block: Block, statement: Statement) -> Self {
        Self {
            block,
            start: BlockStart::Statement(statement),
        }
    }

    pub fn exhausted(block: Block) -> Self {
        Self {
            block,
            start: BlockStart::Exhausted,
        }
    }

    pub fn block(self) -> Block {
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

impl From<Block> for BlockSeed {
    fn from(block: Block) -> Self {
        Self::new(block)
    }
}

/// Public shell seed for entering region-local execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionSeed {
    region: Region,
}

impl RegionSeed {
    pub fn new(region: Region) -> Self {
        Self { region }
    }

    pub fn region(self) -> Region {
        self.region
    }
}

impl From<Region> for RegionSeed {
    fn from(region: Region) -> Self {
        Self::new(region)
    }
}

/// Public shell seed for entering directed-graph execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiGraphSeed {
    digraph: DiGraph,
}

impl DiGraphSeed {
    pub fn new(digraph: DiGraph) -> Self {
        Self { digraph }
    }

    pub fn digraph(self) -> DiGraph {
        self.digraph
    }
}

impl From<DiGraph> for DiGraphSeed {
    fn from(digraph: DiGraph) -> Self {
        Self::new(digraph)
    }
}

/// Public shell seed for entering undirected-graph execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnGraphSeed {
    ungraph: UnGraph,
}

impl UnGraphSeed {
    pub fn new(ungraph: UnGraph) -> Self {
        Self { ungraph }
    }

    pub fn ungraph(self) -> UnGraph {
        self.ungraph
    }
}

impl From<UnGraph> for UnGraphSeed {
    fn from(ungraph: UnGraph) -> Self {
        Self::new(ungraph)
    }
}

/// Public shell execution seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExecutionSeed {
    Block(BlockSeed),
    Region(RegionSeed),
    DiGraph(DiGraphSeed),
    UnGraph(UnGraphSeed),
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

impl From<RegionSeed> for ExecutionSeed {
    fn from(seed: RegionSeed) -> Self {
        Self::Region(seed)
    }
}

impl From<Region> for ExecutionSeed {
    fn from(region: Region) -> Self {
        Self::Region(region.into())
    }
}

impl From<DiGraphSeed> for ExecutionSeed {
    fn from(seed: DiGraphSeed) -> Self {
        Self::DiGraph(seed)
    }
}

impl From<DiGraph> for ExecutionSeed {
    fn from(digraph: DiGraph) -> Self {
        Self::DiGraph(digraph.into())
    }
}

impl From<UnGraphSeed> for ExecutionSeed {
    fn from(seed: UnGraphSeed) -> Self {
        Self::UnGraph(seed)
    }
}

impl From<UnGraph> for ExecutionSeed {
    fn from(ungraph: UnGraph) -> Self {
        Self::UnGraph(ungraph.into())
    }
}
