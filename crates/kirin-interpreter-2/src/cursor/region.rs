use kirin_ir::{Block, Dialect, GetInfo, Region, StageInfo, Statement};

use crate::cursor::BlockCursor;

/// Region-local shell cursor that walks blocks in region order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionCursor {
    region: Region,
    current_block: Option<BlockCursor>,
}

impl RegionCursor {
    pub(crate) fn new<L: Dialect>(stage: &StageInfo<L>, region: Region) -> Self {
        Self {
            region,
            current_block: Self::first_non_empty_block(stage, region)
                .map(|block| BlockCursor::new(stage, block)),
        }
    }

    pub(crate) fn current_block(&self) -> Option<Block> {
        self.current_block.map(|cursor| cursor.block())
    }

    pub(crate) fn current(&self) -> Option<Statement> {
        self.current_block.and_then(|cursor| cursor.current())
    }

    pub(crate) fn advance<L: Dialect>(&mut self, stage: &StageInfo<L>) {
        let Some(mut block_cursor) = self.current_block else {
            return;
        };

        block_cursor.advance(stage);
        if block_cursor.current().is_some() {
            self.current_block = Some(block_cursor);
            return;
        }

        self.current_block = Self::next_non_empty_block(stage, block_cursor.block())
            .map(|block| BlockCursor::new(stage, block));
    }

    fn first_non_empty_block<L: Dialect>(stage: &StageInfo<L>, region: Region) -> Option<Block> {
        region
            .blocks(stage)
            .find(|block| block.first_statement(stage).is_some())
    }

    fn next_non_empty_block<L: Dialect>(stage: &StageInfo<L>, block: Block) -> Option<Block> {
        let mut next = block.expect_info(stage).node.next;
        while let Some(candidate) = next {
            if candidate.first_statement(stage).is_some() {
                return Some(candidate);
            }
            next = candidate.expect_info(stage).node.next;
        }
        None
    }
}
