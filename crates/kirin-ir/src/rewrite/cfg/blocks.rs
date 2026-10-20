use crate::arena::GetInfo;
use crate::{Block, CFG, Dialect};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Create a block with arguments of `arg_types` at the end of `cfg`'s block
    /// list. The block starts empty, with no terminator.
    ///
    /// Records [`MutationEvent::ChangedBlockList`].
    pub fn append_block(
        &mut self,
        cfg: CFG,
        arg_types: impl IntoIterator<Item = L::Type>,
    ) -> Result<Block, RewriteError> {
        let tail = cfg
            .get_info(self.stage)
            .ok_or(RewriteError::UnknownCfg(cfg))?
            .blocks
            .tail;
        Ok(self.create_block(cfg, tail, None, arg_types))
    }

    /// Create a block with arguments of `arg_types` immediately before
    /// `anchor` in its CFG's block list. The block starts empty, with no
    /// terminator.
    ///
    /// Records [`MutationEvent::ChangedBlockList`].
    pub fn insert_block_before(
        &mut self,
        anchor: Block,
        arg_types: impl IntoIterator<Item = L::Type>,
    ) -> Result<Block, RewriteError> {
        let cfg = self.owning_cfg(anchor)?;
        let prev = self.live_block(anchor)?.node.prev;
        Ok(self.create_block(cfg, prev, Some(anchor), arg_types))
    }

    /// Create a block with arguments of `arg_types` immediately after `anchor`
    /// in its CFG's block list. The block starts empty, with no terminator.
    ///
    /// Records [`MutationEvent::ChangedBlockList`].
    pub fn insert_block_after(
        &mut self,
        anchor: Block,
        arg_types: impl IntoIterator<Item = L::Type>,
    ) -> Result<Block, RewriteError> {
        let cfg = self.owning_cfg(anchor)?;
        let next = self.live_block(anchor)?.node.next;
        Ok(self.create_block(cfg, Some(anchor), next, arg_types))
    }

    /// Erase `block`: unlink it from its CFG and tombstone it together with
    /// everything it holds (arguments, statements, results, nested bodies).
    ///
    /// The uses those statements made are dropped and their outgoing edges
    /// are removed from the predecessor mirror.
    ///
    /// Rejects, leaving the stage untouched, if `block` is unknown or not in a
    /// CFG, if another block still branches to it
    /// ([`RewriteError::BlockHasPredecessors`]; retarget those edges first,
    /// a self-loop is fine), or if a value it defines is used outside it
    /// ([`RewriteError::BlockArgumentInUse`] /
    /// [`RewriteError::StatementResultsInUse`]); replace those uses first.
    ///
    /// Records [`MutationEvent::ChangedBlockList`] then
    /// [`MutationEvent::ChangedCfg`].
    pub fn erase_block(&mut self, block: Block) -> Result<(), RewriteError> {
        let cfg = self.owning_cfg(block)?;
        self.check_no_outside_predecessors(block, &[block])?;
        self.check_no_outside_uses(block)?;

        self.unlink_block(cfg, block);
        self.tombstone_block(block);

        self.events.push(MutationEvent::ChangedBlockList { cfg });
        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(())
    }
}
