use crate::arena::GetInfo;
use crate::node::linked_list::{LinkedList, LinkedListNode};
use crate::{Block, Dialect, SSAValue, Statement, StatementInfo, StatementParent, Use};

use crate::rewrite::rewriter::{
    MutationEvent, RewriteError, Rewriter, add_predecessor, add_use, block_is_live,
};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Split the block containing `at` in two: `at`, everything after it, and
    /// the terminator move to a new block placed right after the original,
    /// and the original is closed by the terminator `make_branch` returns.
    ///
    /// `make_branch` receives the new block's id so it can branch to it. The
    /// new block has no arguments. Splitting at the terminator moves only the
    /// terminator. Statement ids, results, and uses are unchanged.
    ///
    /// Rejects, leaving the stage untouched, if `at` is unknown or not in a
    /// CFG block, or if the branch is not a terminator, declares results, or
    /// names a dead operand or successor.
    ///
    /// Records [`MutationEvent::ChangedBlockList`],
    /// [`MutationEvent::InsertedStatement`] for the branch, then
    /// [`MutationEvent::ChangedCfg`].
    pub fn split_block(
        &mut self,
        at: Statement,
        make_branch: impl FnOnce(Block) -> L,
    ) -> Result<Block, RewriteError> {
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

        // The new block's id is known before allocation, so the branch can be
        // built and fully checked before anything is mutated.
        let new_block: Block = self.stage.nodes.blocks.next_id();
        let branch = make_branch(new_block);
        if !branch.is_terminator() {
            return Err(RewriteError::ExpectedTerminator);
        }
        if branch.results().next().is_some() {
            return Err(RewriteError::CannotInsertWithResults);
        }
        let branch_operands: Vec<SSAValue> = branch.arguments().copied().collect();
        for &operand in &branch_operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }
        for successor in branch.successors() {
            let target = successor.target();
            if target != new_block && !block_is_live(self.stage, target) {
                return Err(RewriteError::UnknownBlock(target));
            }
        }

        let successors_before = self.block_successors(block);
        let moved = if at_is_terminator {
            Vec::new()
        } else {
            self.chain_from(Some(at))
        };

        let created = self.create_block(cfg, Some(block), next_block, []);
        debug_assert_eq!(created, new_block);

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

        // Close the original block with the branch.
        let branch_stmt = self
            .stage
            .nodes
            .statements
            .alloc_with_id(|id| StatementInfo {
                node: LinkedListNode::new(id),
                parent: Some(StatementParent::Block(block)),
                definition: branch,
            });
        for (index, operand) in branch_operands.into_iter().enumerate() {
            let site = Use::StatementOperand {
                stmt: branch_stmt,
                index,
            };
            add_use(self.stage, operand, site);
        }
        if let Some(info) = block.get_info_mut(self.stage) {
            info.terminator = Some(branch_stmt);
        }

        // The original block's outgoing edges are now the branch's; the moved
        // edges now leave from the new block.
        let successors_after = self.block_successors(block);
        self.update_predecessors(block, &successors_before, &successors_after);
        for target in self.block_successors(new_block) {
            add_predecessor(self.stage, target, new_block);
        }

        self.events
            .push(MutationEvent::InsertedStatement { stmt: branch_stmt });
        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(new_block)
    }
}
