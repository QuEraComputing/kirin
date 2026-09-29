use crate::arena::GetInfoMut;
use crate::node::linked_list::LinkedList;
use crate::{Block, Dialect, Statement, StatementParent};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter, add_predecessor};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Split the block containing `at` in two: `at`, everything after it, and
    /// the terminator move to a new block placed right after the original,
    /// which is returned.
    ///
    /// The original block is left **without a terminator**; the caller closes
    /// it, typically with a branch to the new block, through
    /// [`Rewriter::insert_terminator`]. The new block has no arguments.
    /// Splitting at the terminator moves only the terminator. Statement ids,
    /// results, and uses are unchanged.
    ///
    /// Rejects, leaving the stage untouched, if `at` is unknown or not in a
    /// CFG block.
    ///
    /// Records [`MutationEvent::ChangedBlockList`] then
    /// [`MutationEvent::ChangedCfg`].
    pub fn split_block(&mut self, at: Statement) -> Result<Block, RewriteError> {
        let (block, at_is_terminator, at_prev) = {
            let item = self.live_statement(at)?;
            let Some(StatementParent::Block(block)) = item.parent else {
                return Err(RewriteError::NotInBlockBody(at));
            };
            (block, item.definition.is_terminator(), item.node.prev)
        };
        let cfg = self.owning_cfg(block)?;
        let (next_block, terminator) = {
            let info = self.live_block(block)?;
            (info.node.next, info.terminator)
        };

        let successors_before = self.block_successors(block);
        let moved = if at_is_terminator {
            Vec::new()
        } else {
            self.chain_from(Some(at))
        };

        let new_block = self.create_block(cfg, Some(block), next_block, []);

        // Cut the moved suffix off the original block's statement chain.
        if !moved.is_empty() {
            if let Some(prev) = at_prev
                && let Some(info) = prev.get_info_mut(self.stage)
            {
                info.node.next = None;
            }
            if let Some(info) = at.get_info_mut(self.stage) {
                info.node.prev = None;
            }
            if let Some(info) = block.get_info_mut(self.stage) {
                info.statements.tail = at_prev;
                if at_prev.is_none() {
                    info.statements.head = None;
                }
                info.statements.len -= moved.len();
            }
        }
        for &stmt in moved.iter().chain(terminator.iter()) {
            if let Some(info) = stmt.get_info_mut(self.stage) {
                info.parent = Some(StatementParent::Block(new_block));
            }
        }
        if let Some(info) = new_block.get_info_mut(self.stage) {
            info.statements = LinkedList {
                head: moved.first().copied(),
                tail: moved.last().copied(),
                len: moved.len(),
            };
            info.terminator = terminator;
        }
        if let Some(info) = block.get_info_mut(self.stage) {
            info.terminator = None;
        }

        // The moved edges now leave from the new block; whatever stays in the
        // original block keeps its edges.
        let successors_after = self.block_successors(block);
        self.update_predecessors(block, &successors_before, &successors_after);
        for target in self.block_successors(new_block) {
            add_predecessor(self.stage, target, new_block);
        }

        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(new_block)
    }
}
