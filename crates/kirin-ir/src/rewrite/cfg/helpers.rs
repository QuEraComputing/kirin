use std::collections::HashSet;

use smallvec::SmallVec;

use crate::arena::GetInfo;
use crate::node::linked_list::LinkedListNode;
use crate::{
    Block, BlockArgument, BlockInfo, BlockParent, CFG, Dialect, SSAInfo, SSAKind, SSAValue,
    Statement, StatementParent, Use,
};

use crate::rewrite::rewriter::{
    MutationEvent, RewriteError, Rewriter, add_predecessor, remove_predecessor,
};

/// Shared lookups and block-list plumbing for the CFG surgery methods.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// The CFG whose block list holds the live block `block`, or
    /// [`RewriteError::UnknownBlock`] / [`RewriteError::NotInCfgBody`].
    pub(super) fn owning_cfg(&self, block: Block) -> Result<CFG, RewriteError> {
        match self.live_block(block)?.parent {
            Some(BlockParent::CFG(cfg)) => Ok(cfg),
            _ => Err(RewriteError::NotInCfgBody(block)),
        }
    }

    /// The statement chain starting at `head`, following `next` links.
    pub(super) fn chain_from(&self, head: Option<Statement>) -> Vec<Statement> {
        let mut out = Vec::new();
        let mut current = head;
        while let Some(stmt) = current {
            out.push(stmt);
            current = stmt.get_info(self.stage).and_then(|item| item.node.next);
        }
        out
    }

    /// Every statement of `block`: the non-terminator chain, then the terminator.
    pub(super) fn block_statements(&self, block: Block) -> Vec<Statement> {
        let Some(info) = block.get_info(self.stage) else {
            return Vec::new();
        };
        let mut out = self.chain_from(info.statements.head);
        out.extend(info.terminator);
        out
    }

    /// The set of blocks `block`'s statements may transfer control to.
    pub(super) fn block_successors(&self, block: Block) -> HashSet<Block> {
        let mut out = HashSet::new();
        for stmt in self.block_statements(block) {
            let definition = &stmt.expect_info(self.stage).definition;
            out.extend(definition.successors().map(|successor| successor.target()));
        }
        out
    }

    /// Bring the predecessor mirror in line after `source`'s successor set
    /// changed from `before` to `after`.
    pub(super) fn update_predecessors(
        &mut self,
        source: Block,
        before: &HashSet<Block>,
        after: &HashSet<Block>,
    ) {
        for &target in before.difference(after) {
            remove_predecessor(self.stage, target, source);
        }
        for &target in after.difference(before) {
            add_predecessor(self.stage, target, source);
        }
    }

    /// Reject removing `block` if a predecessor outside `allowed` still
    /// branches to it, so a rejected removal leaves the stage untouched and an
    /// accepted one leaves no edge naming a tombstoned block.
    pub(super) fn check_no_outside_predecessors(
        &self,
        block: Block,
        allowed: &[Block],
    ) -> Result<(), RewriteError> {
        match self
            .live_block(block)?
            .predecessors
            .iter()
            .find(|pred| !allowed.contains(pred))
        {
            Some(&pred) => Err(RewriteError::BlockHasPredecessors { block, pred }),
            None => Ok(()),
        }
    }

    /// Reject erasing `block` if a value it defines (an argument, or a result
    /// of one of its top-level statements) is used outside it, so a rejected
    /// erase leaves the stage untouched.
    ///
    /// Results of statements nested in `block`'s bodies are not checked: a use
    /// of those outside `block` would already break scoping.
    pub(super) fn check_no_outside_uses(&self, block: Block) -> Result<(), RewriteError> {
        // Cache of blocks known to lie inside `block` (itself included), grown
        // by every walk that reaches it, so later uses in the same nested block
        // are answered by one lookup.
        let mut inside: HashSet<Block> = HashSet::from([block]);
        let info = self.live_block(block)?;
        for (index, &arg) in info.arguments.iter().enumerate() {
            if self.used_outside(arg.into(), &mut inside) {
                return Err(RewriteError::BlockArgumentInUse { block, index });
            }
        }
        for stmt in self.block_statements(block) {
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

    /// Whether some use of `value` lies outside every block in `inside`.
    fn used_outside(&self, value: SSAValue, inside: &mut HashSet<Block>) -> bool {
        value.get_info(self.stage).is_some_and(|info| {
            info.uses()
                .iter()
                .any(|&site| !self.use_is_within(site, inside))
        })
    }

    /// Whether `site` lies in a block of `inside`, directly or inside a body
    /// nested in one.
    ///
    /// Walks up the owner chain (statement → block → CFG or owning statement
    /// → ...) until it reaches a block of `inside` or the root. On success
    /// every block passed on the way is added to `inside`, so any later use in
    /// those blocks stops at the first step. Failures are not cached: the
    /// caller rejects on the first one.
    fn use_is_within(&self, site: Use, inside: &mut HashSet<Block>) -> bool {
        let mut owner = match site {
            Use::StatementOperand { stmt, .. } => Some(stmt),
            Use::DiGraphYield { graph, .. } => graph.get_info(self.stage).and_then(|g| g.parent()),
        };
        let mut path: SmallVec<[Block; 4]> = SmallVec::new();
        while let Some(stmt) = owner {
            owner = match stmt.get_info(self.stage).and_then(|item| item.parent) {
                Some(StatementParent::Block(parent)) => {
                    if inside.contains(&parent) {
                        inside.extend(path);
                        return true;
                    }
                    path.push(parent);
                    match parent.get_info(self.stage).and_then(|item| item.parent) {
                        Some(BlockParent::CFG(cfg)) => {
                            cfg.get_info(self.stage).and_then(|item| item.parent)
                        }
                        Some(BlockParent::Statement(owner)) => Some(owner),
                        None => None,
                    }
                }
                Some(StatementParent::DiGraph(graph)) => {
                    graph.get_info(self.stage).and_then(|g| g.parent())
                }
                Some(StatementParent::UnGraph(graph)) => {
                    graph.get_info(self.stage).and_then(|g| g.parent())
                }
                None => None,
            };
        }
        false
    }

    /// Allocate block argument `index` of `block`.
    pub(super) fn alloc_block_argument(
        &mut self,
        block: Block,
        index: usize,
        ty: L::Type,
    ) -> BlockArgument {
        self.stage
            .ssas
            .alloc_with_id(|id| {
                Some(SSAInfo::new(
                    id,
                    None,
                    ty,
                    SSAKind::BlockArgument(block, index),
                ))
            })
            .into()
    }

    /// Allocate an empty block in `cfg` and link it between `prev` and `next`.
    ///
    /// Records [`MutationEvent::ChangedBlockList`].
    pub(super) fn create_block(
        &mut self,
        cfg: CFG,
        prev: Option<Block>,
        next: Option<Block>,
        arg_types: impl IntoIterator<Item = L::Type>,
    ) -> Block {
        let block = self.stage.nodes.blocks.alloc_with_id(|id| {
            BlockInfo::builder()
                .parent(BlockParent::CFG(cfg))
                .node(LinkedListNode {
                    ptr: id,
                    next,
                    prev,
                })
                .arguments(Vec::new())
                .predecessors(SmallVec::new())
                .new()
        });
        let args: Vec<BlockArgument> = arg_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| self.alloc_block_argument(block, index, ty))
            .collect();
        if let Some(info) = block.get_info_mut(self.stage) {
            info.arguments = args;
        }

        match prev {
            Some(p) => {
                if let Some(info) = p.get_info_mut(self.stage) {
                    info.node.next = Some(block);
                }
            }
            None => {
                if let Some(info) = cfg.get_info_mut(self.stage) {
                    info.blocks.head = Some(block);
                }
            }
        }
        match next {
            Some(n) => {
                if let Some(info) = n.get_info_mut(self.stage) {
                    info.node.prev = Some(block);
                }
            }
            None => {
                if let Some(info) = cfg.get_info_mut(self.stage) {
                    info.blocks.tail = Some(block);
                }
            }
        }
        if let Some(info) = cfg.get_info_mut(self.stage) {
            info.blocks.len += 1;
        }

        self.events.push(MutationEvent::ChangedBlockList { cfg });
        block
    }

    /// Unlink `block` from `cfg`'s block list, clearing its own links.
    pub(super) fn unlink_block(&mut self, cfg: CFG, block: Block) {
        let node = block.expect_info(self.stage).node;
        let (prev, next) = (node.prev, node.next);
        match prev {
            Some(p) => {
                if let Some(info) = p.get_info_mut(self.stage) {
                    info.node.next = next;
                }
            }
            None => {
                if let Some(info) = cfg.get_info_mut(self.stage) {
                    info.blocks.head = next;
                }
            }
        }
        match next {
            Some(n) => {
                if let Some(info) = n.get_info_mut(self.stage) {
                    info.node.prev = prev;
                }
            }
            None => {
                if let Some(info) = cfg.get_info_mut(self.stage) {
                    info.blocks.tail = prev;
                }
            }
        }
        if let Some(info) = cfg.get_info_mut(self.stage) {
            info.blocks.len -= 1;
        }
        if let Some(info) = block.get_info_mut(self.stage) {
            info.node.prev = None;
            info.node.next = None;
        }
    }
}
