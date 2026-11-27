//! Adopting the bodies a definition names.

use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::arena::{GetInfo, GetInfoMut};
use crate::{BlockParent, Body, Dialect, RewriteError, Rewriter, Statement, StatementParent};

impl<L: Dialect> Rewriter<'_, L> {
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
    pub(in crate::rewrite) fn check_new_bodies(
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
    /// owned by nothing, created by this `Rewriter`, and does not contain
    /// `parent`, which would make the statement its own ancestor.
    fn check_can_adopt(&self, body: Body, parent: StatementParent) -> Result<(), RewriteError> {
        if self.body_owner(body)?.is_some() {
            return Err(RewriteError::BodyOwnedElsewhere(body));
        }
        if !self.unowned_bodies.contains(&body) {
            return Err(RewriteError::BodyNotDetached(body));
        }
        if self.is_within(parent, &mut FxHashSet::from_iter([body])) {
            return Err(RewriteError::BodyEnclosesOwner(body));
        }
        Ok(())
    }

    /// The statement owning `body`, or `None` if nothing does. A block in a
    /// CFG belongs to that CFG rather than a statement, so it counts as owned
    /// elsewhere.
    pub(super) fn body_owner(&self, body: Body) -> Result<Option<Statement>, RewriteError> {
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

    /// Make `owner` the parent of `body`, which is no longer detached.
    pub(in crate::rewrite) fn adopt_body(&mut self, body: Body, owner: Statement) {
        self.unowned_bodies.remove(&body);
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
pub(in crate::rewrite) fn bodies_of<L: Dialect>(definition: &L) -> SmallVec<[Body; 2]> {
    definition
        .blocks()
        .map(|&block| Body::Block(block))
        .chain(definition.cfgs().map(|&cfg| Body::CFG(cfg)))
        .chain(definition.digraphs().map(|&graph| Body::DiGraph(graph)))
        .chain(definition.ungraphs().map(|&graph| Body::UnGraph(graph)))
        .collect()
}
