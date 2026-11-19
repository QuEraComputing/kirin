//! Whether one place in the IR is nested in a body.

use crate::arena::GetInfo;
use crate::rewrite::walk::tree;
use crate::{BlockParent, Body, Dialect, Rewriter, Statement, StatementParent};

impl<L: Dialect> Rewriter<'_, L> {
    /// Whether `parent` is `body` or nested in it at any depth.
    pub(in crate::rewrite) fn body_contains(&self, body: Body, parent: StatementParent) -> bool {
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

    /// Whether `stmt` is `owner` or encloses it, at any depth. `owner` is the
    /// statement whose body holds a place, so `true` means placing `stmt`
    /// there would make it its own ancestor.
    ///
    /// Only a statement owning a body can enclose anything, so for the many
    /// that own none this skips the walk up from `owner`.
    pub(in crate::rewrite) fn encloses(&self, stmt: Statement, owner: Option<Statement>) -> bool {
        if !tree::owns_bodies(self.stage, stmt) {
            return false;
        }
        owner.is_some_and(|owner| owner.is_within(self.stage, stmt))
    }
}
