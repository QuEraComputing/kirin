use crate::arena::{GetInfo, GetInfoMut};
use crate::node::linked_list::LinkedList;
use crate::rewrite::walk::tree;
use crate::{Block, BlockParent, CFG, Dialect, StatementParent};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Move every block of `source`, in order, into `anchor`'s CFG,
    /// immediately after `anchor`.
    ///
    /// The blocks keep their ids, arguments, statements and terminators, and
    /// the edges between them stay valid since they all move together.
    /// `source` is left empty and is not erased: its owner still names it.
    /// `source`'s entry block becomes an ordinary block of the destination,
    /// for the caller to branch to.
    ///
    /// Rejects, leaving the stage untouched, if `source` is unknown
    /// ([`RewriteError::UnknownCfg`]), if `anchor` is unknown or not in a CFG
    /// ([`RewriteError::NotInCfgBody`]), or if `anchor` is in `source` or
    /// nested in one of its blocks ([`RewriteError::CfgSplicedIntoItself`]).
    ///
    /// Records [`MutationEvent::ChangedBlockList`] for `source`, then for the
    /// destination, then [`MutationEvent::SplicedCfg`]. A walk visiting a
    /// statement of `source` reports the splice (see [`crate::walk`]).
    pub fn splice_cfg_after(&mut self, source: CFG, anchor: Block) -> Result<(), RewriteError> {
        if source.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownCfg(source));
        }
        let dest = self.owning_cfg(anchor)?;
        // Check `dest` and the CFG of every block enclosing it. If one of them
        // is `source`, its blocks would be spliced into themselves.
        if dest == source {
            return Err(RewriteError::CfgSplicedIntoItself { source, anchor });
        }
        let mut current = dest.get_info(self.stage).and_then(|info| info.parent);
        while let Some(stmt) = current {
            if let Some(StatementParent::Block(block)) = *stmt.parent(self.stage)
                && self.live_block(block)?.parent == Some(BlockParent::CFG(source))
            {
                return Err(RewriteError::CfgSplicedIntoItself { source, anchor });
            }
            current = tree::parent(self.stage, stmt);
        }

        let chain = match source.get_info_mut(self.stage) {
            Some(info) => std::mem::take(&mut info.blocks),
            None => LinkedList::new(),
        };
        let next = anchor.expect_info(self.stage).node.next;
        self.link_blocks(chain, dest, Some(anchor), next);

        self.events
            .push(MutationEvent::ChangedBlockList { cfg: source });
        self.events
            .push(MutationEvent::ChangedBlockList { cfg: dest });
        self.events.push(MutationEvent::SplicedCfg { source, dest });
        Ok(())
    }

    /// Link `chain`, a whole block chain already taken out of its CFG, into
    /// `cfg` between `prev` and `next` (the list head or tail when one is
    /// `None`), making `cfg` the parent of each block.
    fn link_blocks(
        &mut self,
        chain: LinkedList<Block>,
        cfg: CFG,
        prev: Option<Block>,
        next: Option<Block>,
    ) {
        let (Some(head), Some(tail)) = (chain.head, chain.tail) else {
            return;
        };
        let mut current = Some(head);
        while let Some(block) = current {
            let Some(info) = block.get_info_mut(self.stage) else {
                break;
            };
            info.parent = Some(BlockParent::CFG(cfg));
            current = info.node.next;
        }
        if let Some(info) = head.get_info_mut(self.stage) {
            info.node.prev = prev;
        }
        if let Some(info) = tail.get_info_mut(self.stage) {
            info.node.next = next;
        }
        match prev {
            Some(p) => {
                if let Some(info) = p.get_info_mut(self.stage) {
                    info.node.next = Some(head);
                }
            }
            None => {
                if let Some(info) = cfg.get_info_mut(self.stage) {
                    info.blocks.head = Some(head);
                }
            }
        }
        match next {
            Some(n) => {
                if let Some(info) = n.get_info_mut(self.stage) {
                    info.node.prev = Some(tail);
                }
            }
            None => {
                if let Some(info) = cfg.get_info_mut(self.stage) {
                    info.blocks.tail = Some(tail);
                }
            }
        }
        if let Some(info) = cfg.get_info_mut(self.stage) {
            info.blocks.len += chain.len;
        }
    }
}
