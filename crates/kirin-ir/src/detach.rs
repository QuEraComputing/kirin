use crate::arena::GetInfo;
use crate::node::stmt::StatementParent;
use crate::node::{Block, Statement};
use crate::query::{LinkedListElem, LinkedListInfo, ParentInfo};
use crate::{Dialect, StageInfo};

pub trait Detach {
    /// Detach the IR node from its parent.
    fn detach<L: Dialect>(&self, stage: &mut StageInfo<L>);
}

impl Detach for Statement {
    fn detach<L: Dialect>(&self, stage: &mut StageInfo<L>) {
        let (prev, next, parent) = if let Some(info) = self.get_info_mut(stage) {
            let prev = info.get_prev_mut().take();
            let next = info.get_next_mut().take();
            let parent = info.get_parent_mut().take();
            (prev, next, parent)
        } else {
            (None, None, None)
        };

        if let Some(prev) = prev {
            let prev_info = prev.expect_info_mut(stage);
            prev_info.node.next = next;
        }
        if let Some(next) = next {
            let next_info = next.expect_info_mut(stage);
            *next_info.get_prev_mut() = prev;
        }

        // Only Block parents have linked-list structure to update.
        // DiGraph/UnGraph parents store statements in a petgraph, which
        // doesn't use linked-list bookkeeping.
        if let Some(StatementParent::Block(block)) = parent {
            let parent_info = block.expect_info_mut(stage);
            if prev.is_none() {
                debug_assert!(
                    *parent_info.get_head() == Some(*self),
                    "Parent block's head does not match the statement being detached"
                );
                *parent_info.get_head_mut() = next;
            }

            if next.is_none() {
                debug_assert!(
                    *parent_info.get_tail() == Some(*self),
                    "Parent block's tail does not match the statement being detached"
                );
                *parent_info.get_tail_mut() = prev;
            }

            debug_assert!(
                parent_info.get_len() > 0,
                "Parent block length is already zero before detach"
            );
            *parent_info.get_len_mut() -= 1;
        }
    }
}

macro_rules! impl_detach {
    ($ty:ty) => {
        impl Detach for $ty {
            fn detach<L: Dialect>(&self, stage: &mut StageInfo<L>) {
                let (prev, next, parent) = if let Some(info) = self.get_info_mut(stage) {
                    let prev = info.get_prev_mut().take();
                    let next = info.get_next_mut().take();
                    let parent = info.get_parent_mut().take();
                    (prev, next, parent)
                } else {
                    (None, None, None)
                };

                if let Some(prev) = prev {
                    let prev_info = prev.expect_info_mut(stage);
                    prev_info.node.next = next;
                }
                if let Some(next) = next {
                    let next_info = next.expect_info_mut(stage);
                    *next_info.get_prev_mut() = prev;
                }

                if let Some(parent) = parent {
                    let parent_info = parent.expect_info_mut(stage);
                    // if prev is None, set head of parent block to next
                    if prev.is_none() {
                        debug_assert!(
                            *parent_info.get_head() == Some(*self),
                            "Parent block's head does not match the statement being detached"
                        );
                        *parent_info.get_head_mut() = next;
                    }

                    // if next is None, set tail of parent block to prev
                    if next.is_none() {
                        debug_assert!(
                            *parent_info.get_tail() == Some(*self),
                            "Parent block's tail does not match the statement being detached"
                        );
                        *parent_info.get_tail_mut() = prev;
                    }

                    debug_assert!(
                        parent_info.get_len() > 0,
                        "Parent block length is already zero before detach"
                    );
                    *parent_info.get_len_mut() -= 1;
                }
            }
        }
    };
}

impl_detach!(Block);
