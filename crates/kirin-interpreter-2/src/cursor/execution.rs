use kirin_ir::{Block, Dialect, StageInfo, Statement};

use crate::cursor::{BlockCursor, DiGraphCursor, InternalSeed, RegionCursor, UnGraphCursor};

/// Closed internal execution cursor by body shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecutionCursor {
    Block(BlockCursor),
    Region(RegionCursor),
    DiGraph(DiGraphCursor),
    UnGraph(UnGraphCursor),
}

impl ExecutionCursor {
    pub(crate) fn from_seed<L: Dialect>(stage: &StageInfo<L>, seed: InternalSeed) -> Self {
        match seed {
            InternalSeed::Block(seed) => {
                Self::Block(match (seed.starts_at_entry(), seed.start()) {
                    (true, _) => BlockCursor::new(stage, seed.block()),
                    (false, Some(statement)) => BlockCursor::at_statement(seed.block(), statement),
                    (false, None) if seed.is_exhausted() => BlockCursor::exhausted(seed.block()),
                    _ => BlockCursor::new(stage, seed.block()),
                })
            }
            InternalSeed::Region(region) => Self::Region(RegionCursor::new(stage, region)),
            InternalSeed::DiGraph(digraph) => Self::DiGraph(DiGraphCursor::new(stage, digraph)),
            InternalSeed::UnGraph(ungraph) => Self::UnGraph(UnGraphCursor::new(stage, ungraph)),
        }
    }

    pub(crate) fn current(&self) -> Option<Statement> {
        match self {
            Self::Block(cursor) => cursor.current(),
            Self::Region(cursor) => cursor.current(),
            Self::DiGraph(cursor) => cursor.current(),
            Self::UnGraph(cursor) => cursor.current(),
        }
    }

    pub(crate) fn current_block(&self) -> Option<Block> {
        match self {
            Self::Block(cursor) => Some(cursor.block()),
            Self::Region(cursor) => cursor.current_block(),
            Self::DiGraph(_) | Self::UnGraph(_) => None,
        }
    }

    pub(crate) fn advance<L: Dialect>(&mut self, stage: &StageInfo<L>) {
        match self {
            Self::Block(cursor) => cursor.advance(stage),
            Self::Region(cursor) => cursor.advance(stage),
            Self::DiGraph(cursor) => cursor.advance(),
            Self::UnGraph(cursor) => cursor.advance(),
        }
    }
}
