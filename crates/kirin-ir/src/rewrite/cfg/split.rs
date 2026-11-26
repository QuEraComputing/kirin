use crate::node::linked_list::LinkedList;
use crate::{Block, Dialect, Statement, StatementParent};

use crate::rewrite::rewriter::{
    MutationEvent, RewriteError, Rewriter, add_predecessor, remove_predecessor,
};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Split the block containing `at` in two: `at`, everything after it, and
    /// the terminator move to a new block placed right after the original,
    /// which is returned.
    ///
    /// The original block is left **without a terminator**; the caller closes
    /// it, typically with a branch to the new block, through
    /// [`InsertionPoint::Terminator`](crate::InsertionPoint::Terminator). The
    /// new block has no arguments. Splitting at the terminator moves only the
    /// terminator. Statement ids, results, and uses are unchanged.
    ///
    /// Rejects, leaving the stage untouched, if `at` is unknown or not in a
    /// CFG block.
    ///
    /// Costs the number of statements moved plus the terminator's successors:
    /// the statements left in the original block are not read.
    ///
    /// Records [`MutationEvent::ChangedBlockList`] then
    /// [`MutationEvent::ChangedCfg`].
    pub fn split_block(&mut self, at: Statement) -> Result<Block, RewriteError> {
        let (block, at_is_terminator) = {
            let item = self.live_statement(at)?;
            let Some(StatementParent::Block(block)) = item.parent else {
                return Err(RewriteError::NotInBlockBody(at));
            };
            (block, item.definition.is_terminator())
        };
        let cfg = self.owning_cfg(block)?;
        let (next_block, terminator) = {
            let info = self.live_block(block)?;
            (info.node.next, info.terminator)
        };

        // The edges leaving `block` are its terminator's, and they leave from
        // the new block once the terminator moves.
        let targets = self.block_successors(block);
        // Cut `at` and the statements after it off the original block's
        // chain. Splitting at the terminator cuts nothing: it is not in the
        // chain.
        let moved = if at_is_terminator {
            LinkedList::new()
        } else {
            self.split_off(block, at)
        };

        // Give the cut statements and the terminator to a new block right
        // after the original.
        let new_block = self.create_block(cfg, Some(block), next_block, []);
        self.link(moved, new_block, None, None);
        self.set_terminator(new_block, terminator);
        self.set_terminator(block, None);

        for target in targets {
            remove_predecessor(self.stage, target, block);
            add_predecessor(self.stage, target, new_block);
        }

        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(new_block)
    }
}
