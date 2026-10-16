use crate::arena::GetInfo;
use crate::node::linked_list::LinkedList;
use crate::{Block, Dialect, SSAValue, StatementParent};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter, remove_predecessor};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Merge `succ` into `pred`: erase `pred`'s terminator, redirect every use
    /// of `succ`'s `i`-th argument to `arg_replacements[i]`, move `succ`'s
    /// statements and terminator to the end of `pred`, and tombstone `succ`.
    ///
    /// `pred` need not branch to `succ`: any edge `pred`'s terminator carried
    /// elsewhere is dropped.
    ///
    /// Rejects, leaving the stage untouched, if either block is unknown or
    /// not in a CFG, they are in different CFGs or the same block, a block
    /// other than `pred` (including `succ` itself, via a self-loop) still
    /// branches to `succ` ([`RewriteError::BlockHasPredecessors`]; retarget
    /// those edges first),
    /// the replacement count differs from `succ`'s argument count, or a
    /// replacement is not live or is one of `succ`'s own arguments (which the
    /// merge deletes).
    ///
    /// Records [`MutationEvent::ErasedStatement`] for `pred`'s terminator (if
    /// any), the [`Rewriter::replace_all_uses`] events for the argument
    /// redirection, then [`MutationEvent::ChangedBlockList`] and
    /// [`MutationEvent::ChangedCfg`].
    pub fn merge_blocks(
        &mut self,
        pred: Block,
        succ: Block,
        arg_replacements: &[SSAValue],
    ) -> Result<(), RewriteError> {
        let cfg = self.owning_cfg(pred)?;
        if self.owning_cfg(succ)? != cfg {
            return Err(RewriteError::CfgMismatch {
                block: succ,
                expected: cfg,
            });
        }
        if pred == succ {
            return Err(RewriteError::CannotMergeBlockIntoItself(pred));
        }
        // A self-loop on succ is rejected too: its terminator moves into pred
        // and would still target succ, which the merge tombstones.
        self.check_no_outside_predecessors(succ, &[pred])?;
        let succ_args: Vec<SSAValue> = self
            .live_block(succ)?
            .arguments
            .iter()
            .map(|&arg| arg.into())
            .collect();
        if arg_replacements.len() != succ_args.len() {
            return Err(RewriteError::BlockArgumentArityMismatch {
                block: succ,
                expected: succ_args.len(),
                found: arg_replacements.len(),
            });
        }
        for &value in arg_replacements {
            if value.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(value));
            }
            // succ's arguments are tombstoned with succ, so their uses must
            // be redirected to values that survive the merge.
            if succ_args.contains(&value) {
                return Err(RewriteError::ReplacementIsMergedArgument { block: succ, value });
            }
        }

        let pred_successors_before = self.block_successors(pred);
        let succ_successors = self.block_successors(succ);

        if let Some(terminator) = pred.expect_info(self.stage).terminator {
            // The predecessor mirror is reconciled below from successor sets.
            self.tombstone_statement(terminator, None);
            if let Some(info) = pred.get_info_mut(self.stage) {
                info.terminator = None;
            }
            self.events
                .push(MutationEvent::ErasedStatement { stmt: terminator });
        }

        let pairs: Vec<(SSAValue, SSAValue)> = succ_args
            .iter()
            .copied()
            .zip(arg_replacements.iter().copied())
            .filter(|(old, new)| old != new)
            .collect();
        self.redirect_uses(&pairs);

        // Move succ's chain and terminator onto the end of pred.
        let (succ_chain, succ_terminator) = {
            let info = succ.expect_info(self.stage);
            (info.statements, info.terminator)
        };
        let moved = self.chain_from(succ_chain.head);
        for &stmt in moved.iter().chain(succ_terminator.iter()) {
            if let Some(info) = stmt.get_info_mut(self.stage) {
                info.parent = Some(StatementParent::Block(pred));
            }
        }
        if let Some(succ_head) = succ_chain.head {
            match pred.expect_info(self.stage).statements.tail {
                Some(tail) => {
                    if let Some(info) = tail.get_info_mut(self.stage) {
                        info.node.next = Some(succ_head);
                    }
                    if let Some(info) = succ_head.get_info_mut(self.stage) {
                        info.node.prev = Some(tail);
                    }
                }
                None => {
                    if let Some(info) = pred.get_info_mut(self.stage) {
                        info.statements.head = Some(succ_head);
                    }
                }
            }
            if let Some(info) = pred.get_info_mut(self.stage) {
                info.statements.tail = succ_chain.tail;
            }
        }
        if let Some(info) = pred.get_info_mut(self.stage) {
            info.statements.len += succ_chain.len;
            info.terminator = succ_terminator;
        }
        if let Some(info) = succ.get_info_mut(self.stage) {
            info.statements = LinkedList::new();
            info.terminator = None;
        }

        // succ is now empty: tombstoning it only removes the block and its
        // (already redirected) arguments.
        self.unlink_block(cfg, succ);
        self.tombstone_block(succ);

        let pred_successors_after = self.block_successors(pred);
        self.update_predecessors(pred, &pred_successors_before, &pred_successors_after);
        for target in succ_successors {
            remove_predecessor(self.stage, target, succ);
        }

        self.events.push(MutationEvent::ChangedBlockList { cfg });
        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(())
    }
}
