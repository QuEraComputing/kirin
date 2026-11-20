//! Whether one place in the IR is nested in a body.

use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::arena::GetInfo;
use crate::{BlockParent, Body, Dialect, Rewriter, Statement, StatementParent};

impl<L: Dialect> Rewriter<'_, L> {
    /// Whether `place` is one of the bodies in `inside`, or nested in one at
    /// any depth.
    ///
    /// Walks up the owner chain (block → its CFG → owning statement → its
    /// block or graph → ...) until it reaches a body of `inside` or the root.
    /// On success every body passed on the way is added to `inside`, so a
    /// later walk through one of them stops there. Failures are not cached:
    /// callers reject on the first one.
    ///
    /// Reusing one `inside` across many walks, as the erase check does for
    /// every use it looks at, keeps their total cost linear: each step either
    /// stops at a cached body or passes a body that is then cached, so every
    /// nested body is passed at most once.
    pub(super) fn is_within(&self, place: StatementParent, inside: &mut FxHashSet<Body>) -> bool {
        let mut path: SmallVec<[Body; 4]> = SmallVec::new();
        let mut current = place;
        loop {
            // `bodies`: the bodies `current` is part of. That is `current`
            // itself, plus its CFG when `current` is a block in a CFG.
            // `owner`: the statement owning those bodies, if any. The walk
            // continues from that statement's parent.
            let (bodies, owner): (SmallVec<[Body; 2]>, Option<Statement>) = match current {
                StatementParent::Block(block) => {
                    match block.get_info(self.stage).and_then(|item| item.parent) {
                        Some(BlockParent::CFG(cfg)) => (
                            SmallVec::from_slice(&[Body::Block(block), Body::CFG(cfg)]),
                            cfg.get_info(self.stage).and_then(|item| item.parent),
                        ),
                        Some(BlockParent::Statement(owner)) => {
                            (SmallVec::from_slice(&[Body::Block(block)]), Some(owner))
                        }
                        None => (SmallVec::from_slice(&[Body::Block(block)]), None),
                    }
                }
                StatementParent::DiGraph(graph) => (
                    SmallVec::from_slice(&[Body::DiGraph(graph)]),
                    graph.get_info(self.stage).and_then(|item| item.parent()),
                ),
                StatementParent::UnGraph(graph) => (
                    SmallVec::from_slice(&[Body::UnGraph(graph)]),
                    graph.get_info(self.stage).and_then(|item| item.parent()),
                ),
            };
            for body in bodies {
                if inside.contains(&body) {
                    inside.extend(path);
                    return true;
                }
                path.push(body);
            }
            let Some(next) = owner
                .and_then(|stmt| stmt.get_info(self.stage))
                .and_then(|item| item.parent)
            else {
                return false;
            };
            current = next;
        }
    }
}
