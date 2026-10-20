use smallvec::SmallVec;

use crate::arena::{GetInfo, GetInfoMut};
use crate::{
    Block, BlockArgument, Dialect, SSAKind, SSAValue, Statement, StatementParent, Successor,
};

use crate::rewrite::rewriter::{MutationEvent, PreparedReplacement, RewriteError, Rewriter};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Point every successor slot of `stmt` that targets `old` at `new`,
    /// returning the number of slots rewritten.
    ///
    /// `old` need not be live. Forwarded block arguments are not touched.
    /// Rewriting nothing (including `old == new`) records no event.
    ///
    /// Rejects, leaving the stage untouched, if `stmt` is unknown or not in a
    /// CFG block, or `new` is not a live block.
    ///
    /// Records [`MutationEvent::ChangedCfg`].
    pub fn retarget_successor(
        &mut self,
        stmt: Statement,
        old: Block,
        new: Block,
    ) -> Result<usize, RewriteError> {
        let (block, count) = {
            let item = self.live_statement(stmt)?;
            let Some(StatementParent::Block(block)) = item.parent else {
                return Err(RewriteError::NotInBlockBody(stmt));
            };
            let count = item
                .definition
                .successors()
                .filter(|successor| successor.target() == old)
                .count();
            (block, count)
        };
        let cfg = self.owning_cfg(block)?;
        self.live_block(new)?;
        if old == new || count == 0 {
            return Ok(0);
        }

        let before = self.block_successors(block);
        if let Some(item) = stmt.get_info_mut(self.stage) {
            for successor in item.definition.successors_mut() {
                if successor.target() == old {
                    *successor = Successor::from_block(new);
                }
            }
        }
        let after = self.block_successors(block);
        self.update_predecessors(block, &before, &after);

        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(count)
    }

    /// Insert a fresh argument of type `ty` at position `index` of `block`
    /// (`index == len` appends), and insert the matching operand into every
    /// incoming edge.
    ///
    /// Edges are found through [`BranchOp`](crate::BranchOp): for each
    /// statement (in a predecessor block) that has `block` as a successor,
    /// `forwarded(stmt, successor)` gives the value that edge passes for the
    /// new argument, where `successor` is the edge's position in
    /// `successors()`. Every rewritten edge is checked as by
    /// [`Rewriter::replace_statement`] before any edit is made. Returns the new
    /// argument.
    ///
    /// Rejects, leaving the stage untouched, if `index` is past the end of the
    /// argument list, an incoming edge has no editable operand list
    /// ([`RewriteError::NoSuccessorOperands`]), an edge's operand list is
    /// shorter than `index`, or a forwarded value is not live.
    ///
    /// Records one [`MutationEvent::ReplacedStatement`] per rewritten edge
    /// statement, then [`MutationEvent::ChangedCfg`].
    pub fn insert_block_arg(
        &mut self,
        block: Block,
        index: usize,
        ty: L::Type,
        mut forwarded: impl FnMut(Statement, usize) -> SSAValue,
    ) -> Result<BlockArgument, RewriteError> {
        let cfg = self.owning_cfg(block)?;
        if index > self.live_block(block)?.arguments.len() {
            return Err(RewriteError::BlockArgumentIndexOutOfRange { block, index });
        }
        let edges = self.prepare_incoming_edges(block, |stmt, successor, operands| {
            if index > operands.len() {
                return Err(RewriteError::SuccessorOperandIndexOutOfRange {
                    stmt,
                    successor,
                    index,
                });
            }
            operands.insert(index, forwarded(stmt, successor));
            Ok(())
        })?;

        let arg = self.alloc_block_argument(block, index, ty);
        if let Some(info) = block.get_info_mut(self.stage) {
            info.arguments.insert(index, arg);
        }
        self.reindex_block_args(block, index + 1);

        self.commit_incoming_edges(edges);
        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(arg)
    }

    /// Remove argument `index` of `block`, tombstoning it, and remove the
    /// matching operand from every incoming edge (found through
    /// [`BranchOp`](crate::BranchOp)). The argument must have no uses.
    ///
    /// Rejects, leaving the stage untouched, if `index` is out of range, the
    /// argument is still used, an incoming edge has no editable operand list,
    /// or an edge's operand list has no operand at `index`.
    ///
    /// Records one [`MutationEvent::ReplacedStatement`] per rewritten edge
    /// statement, then [`MutationEvent::ChangedCfg`].
    pub fn erase_block_arg(&mut self, block: Block, index: usize) -> Result<(), RewriteError> {
        let cfg = self.owning_cfg(block)?;
        let arg: SSAValue = self
            .live_block(block)?
            .arguments
            .get(index)
            .copied()
            .ok_or(RewriteError::BlockArgumentIndexOutOfRange { block, index })?
            .into();
        if arg
            .get_info(self.stage)
            .is_some_and(|info| !info.uses().is_empty())
        {
            return Err(RewriteError::BlockArgumentInUse { block, index });
        }
        let edges = self.prepare_incoming_edges(block, |stmt, successor, operands| {
            if index >= operands.len() {
                return Err(RewriteError::SuccessorOperandIndexOutOfRange {
                    stmt,
                    successor,
                    index,
                });
            }
            operands.remove(index);
            Ok(())
        })?;

        if let Some(info) = block.get_info_mut(self.stage) {
            info.arguments.remove(index);
        }
        let _ = self.stage.ssas.delete(arg);
        self.reindex_block_args(block, index);

        self.commit_incoming_edges(edges);
        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(())
    }

    /// For every statement that branches to `block`, apply `edit` to the
    /// operand list it forwards on each such edge (a statement may target
    /// `block` more than once), then check the edited definition, without
    /// mutating the stage.
    fn prepare_incoming_edges(
        &self,
        block: Block,
        mut edit: impl FnMut(Statement, usize, &mut Vec<SSAValue>) -> Result<(), RewriteError>,
    ) -> Result<Vec<(Statement, PreparedReplacement, L)>, RewriteError> {
        let mut edges = Vec::new();
        for &pred in &self.live_block(block)?.predecessors {
            for stmt in self.block_statements(pred) {
                let definition = &stmt.expect_info(self.stage).definition;
                let positions: SmallVec<[usize; 2]> = definition
                    .successors()
                    .enumerate()
                    .filter(|(_, successor)| successor.target() == block)
                    .map(|(position, _)| position)
                    .collect();
                if positions.is_empty() {
                    continue;
                }
                let mut definition = definition.clone();
                for successor in positions {
                    let operands = definition
                        .successor_operands_mut(successor)
                        .ok_or(RewriteError::NoSuccessorOperands { stmt, successor })?;
                    edit(stmt, successor, operands)?;
                }
                let prepared = self.prepare_replacement(stmt, &definition)?;
                edges.push((stmt, prepared, definition));
            }
        }
        Ok(edges)
    }

    fn commit_incoming_edges(&mut self, edges: Vec<(Statement, PreparedReplacement, L)>) {
        for (stmt, prepared, definition) in edges {
            self.commit_replacement(stmt, prepared, definition);
        }
    }

    /// Bring the `SSAKind` positions of `block`'s arguments from `start` on in
    /// line with their place in the argument list.
    fn reindex_block_args(&mut self, block: Block, start: usize) {
        let args = block.expect_info(self.stage).arguments.clone();
        for (position, &arg) in args.iter().enumerate().skip(start) {
            if let Some(info) = SSAValue::from(arg).get_info_mut(self.stage) {
                info.kind = SSAKind::BlockArgument(block, position);
            }
        }
    }
}
