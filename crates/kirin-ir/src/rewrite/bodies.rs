//! Bodies owned by a statement, for [`Rewriter::replace_statement`] and
//! statement erasure.

use smallvec::SmallVec;

use crate::arena::{GetInfo, GetInfoMut};
use crate::{BlockParent, Body, Dialect, RewriteError, Rewriter, Statement, StatementParent};

impl<L: Dialect> Rewriter<'_, L> {
    /// Tombstone `body` and everything nested in it.
    pub(super) fn tombstone_body(&mut self, body: Body) {
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

    /// Check that `owner` can adopt `body`: it is live, owned by nothing else,
    /// and does not enclose `owner`.
    pub(super) fn check_can_adopt(&self, body: Body, owner: Statement) -> Result<(), RewriteError> {
        match self.body_owner(body)? {
            Some(other) if other != owner => Err(RewriteError::BodyOwnedElsewhere(body)),
            Some(_) => Ok(()),
            // Owning a body it is nested in would make `owner` its own
            // ancestor. An owned body enclosing `owner` belongs to one of its
            // ancestors and is rejected above, so only an unowned one needs
            // the walk up.
            None if self.body_encloses(body, owner) => Err(RewriteError::BodyEnclosesOwner(body)),
            None => Ok(()),
        }
    }

    /// The statement owning `body`, or `None` if nothing does. A block in a
    /// CFG belongs to that CFG rather than a statement, so it counts as owned
    /// elsewhere.
    fn body_owner(&self, body: Body) -> Result<Option<Statement>, RewriteError> {
        let unknown = RewriteError::UnknownBody(body);
        match body {
            Body::Block(block) => match block.get_info(self.stage).ok_or(unknown)?.parent {
                Some(BlockParent::CFG(_)) => Err(RewriteError::BodyOwnedElsewhere(body)),
                Some(BlockParent::Statement(stmt)) => Ok(Some(stmt)),
                None => Ok(None),
            },
            Body::CFG(cfg) => Ok(cfg.get_info(self.stage).ok_or(unknown)?.parent),
            Body::DiGraph(graph) => Ok(graph.get_info(self.stage).ok_or(unknown)?.parent()),
            Body::UnGraph(graph) => Ok(graph.get_info(self.stage).ok_or(unknown)?.parent()),
        }
    }

    /// Whether `stmt` is nested, at any depth, in `body`.
    fn body_encloses(&self, body: Body, stmt: Statement) -> bool {
        let mut current = stmt;
        while let Some(parent) = current.get_info(self.stage).and_then(|item| item.parent) {
            let owner = match parent {
                StatementParent::Block(block) => {
                    if body == Body::Block(block) {
                        return true;
                    }
                    match block.get_info(self.stage).and_then(|item| item.parent) {
                        Some(BlockParent::CFG(cfg)) => {
                            if body == Body::CFG(cfg) {
                                return true;
                            }
                            cfg.get_info(self.stage).and_then(|item| item.parent)
                        }
                        Some(BlockParent::Statement(owner)) => Some(owner),
                        None => None,
                    }
                }
                StatementParent::DiGraph(graph) => {
                    if body == Body::DiGraph(graph) {
                        return true;
                    }
                    graph.get_info(self.stage).and_then(|item| item.parent())
                }
                StatementParent::UnGraph(graph) => {
                    if body == Body::UnGraph(graph) {
                        return true;
                    }
                    graph.get_info(self.stage).and_then(|item| item.parent())
                }
            };
            let Some(owner) = owner else {
                return false;
            };
            current = owner;
        }
        false
    }

    /// Make `owner` the parent of `body`.
    pub(super) fn adopt_body(&mut self, body: Body, owner: Statement) {
        match body {
            Body::Block(block) => {
                if let Some(item) = block.get_info_mut(self.stage) {
                    item.parent = Some(BlockParent::Statement(owner));
                }
            }
            Body::CFG(cfg) => {
                if let Some(item) = cfg.get_info_mut(self.stage) {
                    item.parent = Some(owner);
                }
            }
            Body::DiGraph(graph) => {
                if let Some(item) = graph.get_info_mut(self.stage) {
                    item.parent = Some(owner);
                }
            }
            Body::UnGraph(graph) => {
                if let Some(item) = graph.get_info_mut(self.stage) {
                    item.parent = Some(owner);
                }
            }
        }
    }
}

/// The bodies `definition` names, in order: `Block`s, `CFG`s, `DiGraph`s,
/// then `UnGraph`s.
pub(super) fn bodies_of<L: Dialect>(definition: &L) -> SmallVec<[Body; 2]> {
    definition
        .blocks()
        .map(|&block| Body::Block(block))
        .chain(definition.cfgs().map(|&cfg| Body::CFG(cfg)))
        .chain(definition.digraphs().map(|&graph| Body::DiGraph(graph)))
        .chain(definition.ungraphs().map(|&graph| Body::UnGraph(graph)))
        .collect()
}
