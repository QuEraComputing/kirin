//! Tombstoning bodies.

use crate::arena::GetInfo;
use crate::{Body, Dialect, Rewriter};

impl<L: Dialect> Rewriter<'_, L> {
    /// Tombstone `body` and everything nested in it.
    pub(in crate::rewrite) fn tombstone_body(&mut self, body: Body) {
        match body {
            Body::Block(block) => self.tombstone_block(block),
            Body::CFG(cfg) => {
                let mut current = cfg.get_info(self.stage).and_then(|item| item.blocks.head);
                while let Some(block) = current {
                    current = block.get_info(self.stage).and_then(|item| item.node.next);
                    self.tombstone_block(block);
                }
                let _ = self.stage.nodes.cfgs.delete(cfg);
            }
            Body::DiGraph(graph) => self.tombstone_digraph(graph),
            Body::UnGraph(graph) => self.tombstone_ungraph(graph),
        }
    }
}
