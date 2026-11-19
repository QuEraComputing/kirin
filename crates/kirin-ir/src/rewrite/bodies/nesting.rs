//! Whether one place in the IR is nested in a body.

use crate::arena::GetInfo;
use crate::{BlockParent, Body, Dialect, Rewriter, StatementParent};

impl<L: Dialect> Rewriter<'_, L> {
    /// Whether `parent` is `body` or nested in it at any depth.
    pub(super) fn body_contains(&self, body: Body, parent: StatementParent) -> bool {
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
}
