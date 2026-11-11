//! Bodies owned by a statement: creating a block for one, and adopting or
//! tombstoning bodies for [`Rewriter::replace_statement`] and statement
//! erasure.

use smallvec::SmallVec;

use crate::arena::{GetInfo, GetInfoMut};
use crate::{
    Block, BlockParent, Body, Dialect, RewriteError, Rewriter, Statement, StatementParent,
};

impl<L: Dialect> Rewriter<'_, L> {
    /// Create an empty block with arguments of `arg_types`, owned by nothing.
    ///
    /// The block is meant to become a statement's body, such as a branch of
    /// an `scf.if`: fill it, then name it in a definition passed to an insert
    /// (such as [`Rewriter::insert_before`]) or to
    /// [`Rewriter::replace_statement`], which makes the statement its parent.
    /// Statements can be added to it like any block, starting with its
    /// terminator ([`Rewriter::insert_terminator`]).
    ///
    /// Until the block is attached or erased, its statements count as uses of
    /// the values they read: such a value cannot be erased, dead-code
    /// elimination keeps it, and [`Rewriter::replace_all_uses`] rewrites those
    /// uses too. To abandon the block early, call [`Rewriter::erase_block`].
    ///
    /// A block no statement owns when the `Rewriter` is dropped is tombstoned
    /// together with everything it holds, unless the drop follows a panic, in
    /// which case it is left for inspection. Unlike [`Rewriter::erase_block`],
    /// the drop does not check that nothing outside still refers to the block:
    /// a branch to it, or a use of a value it defines, is left dangling. When
    /// a pass run by [`Program::run_pass`](crate::Program::run_pass) succeeds,
    /// [`verify_derived`](crate::verify_derived) then finds it and the stage is
    /// quarantined. When the pass fails, or the `Rewriter` is used directly,
    /// nothing reports it.
    ///
    /// Records no event: a block owned by nothing is not part of the IR yet.
    /// Edits inside it record their usual events. The statement that adopts it
    /// records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// or [`MutationEvent::ChangedBodyNesting`](crate::MutationEvent::ChangedBodyNesting).
    pub fn new_block(&mut self, arg_types: impl IntoIterator<Item = L::Type>) -> Block {
        let block = self.alloc_block(None, arg_types);
        self.unowned_blocks.insert(block);
        block
    }

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

    /// Check the bodies a definition names, for a statement under `parent`.
    ///
    /// `bodies` is every body the definition names, in order. `kept` is the
    /// bodies the statement already owns and keeps: empty for a statement
    /// being inserted, and the old definition's bodies for a replacement.
    ///
    /// Each body in `bodies` must be named once
    /// ([`RewriteError::BodyListedTwice`]), and each one not in `kept` must be
    /// adoptable by a statement under `parent` (see
    /// [`check_can_adopt`](Self::check_can_adopt)).
    pub(super) fn check_new_bodies(
        &self,
        bodies: &[Body],
        kept: &[Body],
        parent: StatementParent,
    ) -> Result<(), RewriteError> {
        for (index, &body) in bodies.iter().enumerate() {
            if bodies[..index].contains(&body) {
                return Err(RewriteError::BodyListedTwice(body));
            }
            if !kept.contains(&body) {
                self.check_can_adopt(body, parent)?;
            }
        }
        Ok(())
    }

    /// Check that a statement under `parent` can adopt `body`: it is live,
    /// owned by nothing, and does not contain `parent`, which would make the
    /// statement its own ancestor.
    fn check_can_adopt(&self, body: Body, parent: StatementParent) -> Result<(), RewriteError> {
        if self.body_owner(body)?.is_some() {
            return Err(RewriteError::BodyOwnedElsewhere(body));
        }
        if self.body_contains(body, parent) {
            return Err(RewriteError::BodyEnclosesOwner(body));
        }
        Ok(())
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

    /// Whether `parent` is `body` or nested in it at any depth.
    fn body_contains(&self, body: Body, parent: StatementParent) -> bool {
        let mut current = parent;
        loop {
            let owner = match current {
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
                // A module is not a body and no statement owns it.
                StatementParent::Module(_) => None,
            };
            let Some(owner) = owner else {
                return false;
            };
            let Some(next) = owner.get_info(self.stage).and_then(|item| item.parent) else {
                return false;
            };
            current = next;
        }
    }

    /// Make `owner` the parent of `body`.
    pub(super) fn adopt_body(&mut self, body: Body, owner: Statement) {
        match body {
            Body::Block(block) => {
                self.unowned_blocks.remove(&block);
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
