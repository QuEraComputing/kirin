//! Erasing and tombstoning bodies.

use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::arena::GetInfo;
use crate::{
    Block, BlockParent, Body, CFG, Dialect, MutationEvent, RewriteError, Rewriter, SSAValue,
    Statement, StatementParent, Use,
};

impl<L: Dialect> Rewriter<'_, L> {
    /// Erase `body`: tombstone it together with everything it holds
    /// (arguments, ports, statements, results, nested bodies). The uses those
    /// statements made are dropped and their outgoing edges are removed from
    /// the predecessor mirror.
    ///
    /// `body` must be one of:
    /// - a block in a CFG, which is unlinked from the CFG's block list;
    /// - a body this `Rewriter` created that no statement owns yet, such as a
    ///   [`Rewriter::new_block`] block a pattern gives up on.
    ///
    /// A body owned by a statement is erased with that statement
    /// ([`Rewriter::erase_statement`]) or dropped from it by
    /// [`Rewriter::replace_statement`].
    ///
    /// Rejects, leaving the stage untouched, if:
    /// - `body` is unknown ([`RewriteError::UnknownBody`]);
    /// - a statement owns it ([`RewriteError::BodyOwnedElsewhere`]);
    /// - nothing owns it but this `Rewriter` did not create it
    ///   ([`RewriteError::BodyNotDetached`]);
    /// - a block outside it branches to one of its blocks
    ///   ([`RewriteError::BlockHasPredecessors`]); retarget those edges first.
    ///   Branches between its own blocks, such as a self-loop, are fine;
    /// - a value it defines is used outside it
    ///   ([`RewriteError::BlockArgumentInUse`] /
    ///   [`RewriteError::StatementResultsInUse`] /
    ///   [`RewriteError::PortInUse`]); replace those uses first.
    ///
    /// For a block in a CFG, records [`MutationEvent::ChangedBlockList`] then
    /// [`MutationEvent::ChangedCfg`]. A body no statement owns is not part of
    /// the IR, so erasing it records no event.
    pub fn erase_body(&mut self, body: impl Into<Body>) -> Result<(), RewriteError> {
        let body = body.into();
        let cfg = self.erasable_from(body)?;
        self.check_no_outside_references(body)?;

        match (body, cfg) {
            (Body::Block(block), Some(cfg)) => {
                self.unlink(block, cfg);
                self.tombstone_block(block);
                self.events.push(MutationEvent::ChangedBlockList { cfg });
                self.events.push(MutationEvent::ChangedCfg { cfg });
            }
            _ => {
                self.unowned_bodies.remove(&body);
                self.tombstone_body(body);
            }
        }
        Ok(())
    }

    /// Check that `body` can be erased on its own, returning the CFG to unlink
    /// it from if it is a block in one, or `None` if it is a body this
    /// `Rewriter` created that nothing owns.
    fn erasable_from(&self, body: Body) -> Result<Option<CFG>, RewriteError> {
        if let Body::Block(block) = body {
            let info = block
                .get_info(self.stage)
                .ok_or(RewriteError::UnknownBody(body))?;
            if let Some(BlockParent::CFG(cfg)) = info.parent {
                return Ok(Some(cfg));
            }
        }
        if self.body_owner(body)?.is_some() {
            return Err(RewriteError::BodyOwnedElsewhere(body));
        }
        if !self.unowned_bodies.contains(&body) {
            return Err(RewriteError::BodyNotDetached(body));
        }
        Ok(None)
    }

    /// Reject erasing `body` if a block outside it branches into it, or a
    /// value it defines is used outside it.
    ///
    /// The values checked are those `body` defines at its top level: block
    /// arguments, graph ports, and results of its blocks' statements or graph
    /// members. Results of statements nested deeper are not checked: a use of
    /// those outside `body` would already break scoping.
    fn check_no_outside_references(&self, body: Body) -> Result<(), RewriteError> {
        let blocks: SmallVec<[Block; 4]> = match body {
            Body::Block(block) => SmallVec::from_slice(&[block]),
            Body::CFG(cfg) => {
                let mut blocks = SmallVec::new();
                let mut current = cfg.get_info(self.stage).and_then(|item| item.blocks.head);
                while let Some(block) = current {
                    blocks.push(block);
                    current = block.get_info(self.stage).and_then(|item| item.node.next);
                }
                blocks
            }
            Body::DiGraph(_) | Body::UnGraph(_) => SmallVec::new(),
        };
        // A set, so checking each predecessor stays constant-time even for a
        // CFG with many blocks.
        let own_blocks: FxHashSet<Block> = blocks.iter().copied().collect();
        for &block in &blocks {
            let outside = self
                .live_block(block)?
                .predecessors
                .iter()
                .find(|pred| !own_blocks.contains(pred));
            if let Some(&pred) = outside {
                return Err(RewriteError::BlockHasPredecessors { block, pred });
            }
        }

        let mut inside = FxHashSet::from_iter([body]);
        let mut members: Vec<Statement> = Vec::new();
        for &block in &blocks {
            let info = self.live_block(block)?;
            for (index, &arg) in info.arguments.iter().enumerate() {
                if self.used_outside(arg.into(), &mut inside) {
                    return Err(RewriteError::BlockArgumentInUse { block, index });
                }
            }
            members.extend(self.block_statements(block));
        }
        let ports = match body {
            Body::DiGraph(graph) => {
                let info = self.live_digraph(graph)?;
                members.extend(info.members().map(|(stmt, _)| stmt));
                info.ports().to_vec()
            }
            Body::UnGraph(graph) => {
                let info = self.live_ungraph(graph)?;
                members.extend(info.members().map(|(stmt, _)| stmt));
                info.ports().to_vec()
            }
            Body::Block(_) | Body::CFG(_) => Vec::new(),
        };
        for port in ports {
            if self.used_outside(port.into(), &mut inside) {
                return Err(RewriteError::PortInUse(port));
            }
        }
        for stmt in members {
            let definition = &stmt.expect_info(self.stage).definition;
            if definition
                .results()
                .any(|&result| self.used_outside(result.into(), &mut inside))
            {
                return Err(RewriteError::StatementResultsInUse(stmt));
            }
        }
        Ok(())
    }

    /// Whether some use of `value` lies outside every body in `inside`.
    fn used_outside(&self, value: SSAValue, inside: &mut FxHashSet<Body>) -> bool {
        value.get_info(self.stage).is_some_and(|info| {
            info.uses().iter().any(|&site| {
                let place = match site {
                    Use::StatementOperand { stmt, .. } => {
                        stmt.get_info(self.stage).and_then(|item| item.parent)
                    }
                    Use::DiGraphYield { graph, .. } => Some(StatementParent::DiGraph(graph)),
                };
                !place.is_some_and(|place| self.is_within(place, inside))
            })
        })
    }

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
