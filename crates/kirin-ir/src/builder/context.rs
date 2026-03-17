use super::block::BlockBuilder;
use super::digraph::DiGraphBuilder;
use super::region::RegionBuilder;
use super::ungraph::UnGraphBuilder;

use crate::arena::GetInfo;
use crate::node::*;
use crate::{Dialect, StageInfo};

impl<L: Dialect> StageInfo<L> {
    pub fn block(&mut self) -> BlockBuilder<'_, L> {
        BlockBuilder::from_stage(self)
    }

    pub fn region(&mut self) -> RegionBuilder<'_, L> {
        RegionBuilder::from_stage(self)
    }

    pub fn digraph(&mut self) -> DiGraphBuilder<'_, L> {
        DiGraphBuilder::from_stage(self)
    }

    pub fn ungraph(&mut self) -> UnGraphBuilder<'_, L> {
        UnGraphBuilder::from_stage(self)
    }

    pub fn link_statements(&mut self, ptrs: &[Statement]) -> LinkedList<Statement> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_stmt = current.expect_info_mut(self);
            if let Some(next) = current_stmt.node.next {
                let info = next.expect_info(self);
                panic!("Statement already has a next node: {:?}", info.definition);
            }
            current_stmt.node.next = Some(next);

            let next_stmt = next.expect_info_mut(self);
            if let Some(prev) = next_stmt.node.prev {
                let info = prev.expect_info(self);
                panic!(
                    "Statement already has a previous node: {:?}",
                    info.definition
                );
            }
            next_stmt.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }

    pub fn link_blocks(&mut self, ptrs: &[Block]) -> LinkedList<Block> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_block = current.expect_info_mut(self);
            if let Some(next) = current_block.node.next {
                let info = next.expect_info(self);
                panic!("Block already has a next node: {:?}", info);
            }
            current_block.node.next = Some(next);

            let next_block = next.expect_info_mut(self);
            if let Some(prev) = next_block.node.prev {
                let info = prev.expect_info(self);
                panic!("Block already has a previous node: {:?}", info);
            }
            next_block.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }
}
