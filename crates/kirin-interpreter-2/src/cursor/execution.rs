use kirin_ir::{Block, Dialect, StageInfo, Statement};

use crate::{
    ExecutionSeed,
    cursor::{BlockCursor, DiGraphCursor, RegionCursor, UnGraphCursor},
};

/// Closed internal execution cursor by body shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecutionCursor {
    Block(BlockCursor),
    Region(RegionCursor),
    DiGraph(DiGraphCursor),
    UnGraph(UnGraphCursor),
}

impl ExecutionCursor {
    pub(crate) fn from_seed<L: Dialect>(stage: &StageInfo<L>, seed: ExecutionSeed) -> Self {
        match seed {
            ExecutionSeed::Block(seed) => Self::Block(BlockCursor::new(stage, seed.block())),
            ExecutionSeed::Region(seed) => Self::Region(RegionCursor::new(stage, seed.region())),
            ExecutionSeed::DiGraph(seed) => {
                Self::DiGraph(DiGraphCursor::new(stage, seed.digraph()))
            }
            ExecutionSeed::UnGraph(seed) => {
                Self::UnGraph(UnGraphCursor::new(stage, seed.ungraph()))
            }
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
