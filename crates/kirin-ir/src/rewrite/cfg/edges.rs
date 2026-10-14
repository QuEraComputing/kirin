use crate::arena::GetInfo;
use crate::{
    Block, BlockArgument, Dialect, SSAKind, SSAValue, Statement, StatementParent, Successor,
};

use crate::rewrite::rewriter::{
    MutationEvent, PreparedReplacement, RewriteError, Rewriter, block_is_live,
};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Point every successor slot of `stmt` that targets `old` at `new`,
    /// returning the number of slots rewritten.
    ///
    /// `old` need not be live, so this can also repair an edge left dangling
    /// by [`Rewriter::erase_block`]. Forwarded block arguments are not
    /// touched. Rewriting nothing (including `old == new`) records no event.
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
        if !block_is_live(self.stage, new) {
            return Err(RewriteError::UnknownBlock(new));
        }
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
    /// (`index == len` appends), and rewrite every incoming edge to match.
    ///
    /// Which operands an edge forwards to which successor is dialect-specific,
    /// so `fix_edge` is called once per statement (in a predecessor block)
    /// that has `block` as a successor, with that statement and its current
    /// definition, and returns the rewritten definition. Every rewritten
    /// definition is checked as by [`Rewriter::replace_statement`] before any
    /// edit is made. Returns the new argument.
    ///
    /// Records one [`MutationEvent::ReplacedStatement`] per rewritten edge
    /// statement, then [`MutationEvent::ChangedCfg`].
    pub fn insert_block_arg(
        &mut self,
        block: Block,
        index: usize,
        ty: L::Type,
        fix_edge: impl FnMut(Statement, &L) -> L,
    ) -> Result<BlockArgument, RewriteError> {
        let cfg = self.owning_cfg(block)?;
        if index > self.live_block(block)?.arguments.len() {
            return Err(RewriteError::BlockArgumentIndexOutOfRange { block, index });
        }
        let edges = self.prepare_incoming_edges(block, fix_edge)?;

        let arg = self.alloc_block_argument(block, index, ty);
        if let Some(info) = block.get_info_mut(self.stage) {
            info.arguments.insert(index, arg);
        }
        self.reindex_block_args(block, index + 1);

        self.commit_incoming_edges(edges);
        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(arg)
    }

    /// Remove argument `index` of `block`, tombstoning it, and rewrite every
    /// incoming edge to match. The argument must have no uses.
    ///
    /// `fix_edge` works as for [`Rewriter::insert_block_arg`].
    ///
    /// Records one [`MutationEvent::ReplacedStatement`] per rewritten edge
    /// statement, then [`MutationEvent::ChangedCfg`].
    pub fn erase_block_arg(
        &mut self,
        block: Block,
        index: usize,
        fix_edge: impl FnMut(Statement, &L) -> L,
    ) -> Result<(), RewriteError> {
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
        let edges = self.prepare_incoming_edges(block, fix_edge)?;

        if let Some(info) = block.get_info_mut(self.stage) {
            info.arguments.remove(index);
        }
        let _ = self.stage.ssas.delete(arg);
        self.reindex_block_args(block, index);

        self.commit_incoming_edges(edges);
        self.events.push(MutationEvent::ChangedCfg { cfg });
        Ok(())
    }

    /// Rewrite every statement that branches to `block` with `fix_edge` and
    /// check each result, without mutating.
    fn prepare_incoming_edges(
        &self,
        block: Block,
        mut fix_edge: impl FnMut(Statement, &L) -> L,
    ) -> Result<Vec<(Statement, PreparedReplacement, L)>, RewriteError> {
        let mut edges = Vec::new();
        for &pred in &self.live_block(block)?.predecessors {
            for stmt in self.block_statements(pred) {
                let definition = &stmt.expect_info(self.stage).definition;
                if definition.successors().any(|s| s.target() == block) {
                    let definition = fix_edge(stmt, definition);
                    let prepared = self.prepare_replacement(stmt, &definition)?;
                    edges.push((stmt, prepared, definition));
                }
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
