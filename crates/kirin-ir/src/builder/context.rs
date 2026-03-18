use super::block::BlockBuilder;
use super::digraph::DiGraphBuilder;
use super::region::RegionBuilder;
use super::ungraph::UnGraphBuilder;

use crate::{BuilderStageInfo, Dialect};

impl<L: Dialect> BuilderStageInfo<L> {
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
}
