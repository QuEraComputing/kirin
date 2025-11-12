use crate::node::{Block, StatementRef};
use crate::query::{Info, LinkedListElem, LinkedListInfo, ParentInfo};
use crate::{Arena, Language};

pub trait Detach {
    /// Detach the IR node from its parent.
    fn detach<L: Language>(&self, ctx: &mut Arena<L>) -> eyre::Result<()>;
}

macro_rules! impl_detach {
    ($ty:ty) => {
        impl Detach for $ty {
            fn detach<L: Language>(&self, ctx: &mut Arena<L>) -> eyre::Result<()> {
                let (prev, next, parent) = if let Some(info) = self.get_info_mut(ctx) {
                    let prev = info.get_prev_mut().take();
                    let next = info.get_next_mut().take();
                    let parent = info.get_parent_mut().take();
                    (prev, next, parent)
                } else {
                    (None, None, None)
                };

                prev.and_then(|prev| {
                    let prev_info = prev.expect_info_mut(ctx);
                    prev_info.node.next = next;
                    Some(())
                });
                next.and_then(|next| {
                    let next_info = next.expect_info_mut(ctx);
                    *next_info.get_prev_mut() = prev;
                    Some(())
                });

                if let Some(parent) = parent {
                    let parent_info = parent.expect_info_mut(ctx);
                    // if prev is None, set head of parent block to next
                    if let None = prev {
                        debug_assert!(
                            *parent_info.get_head() == Some(*self),
                            "Parent block's head does not match the statement being detached"
                        );
                        *parent_info.get_head_mut() = next;
                    }

                    // if next is None, set tail of parent block to prev
                    if let None = next {
                        debug_assert!(
                            *parent_info.get_tail() == Some(*self),
                            "Parent block's tail does not match the statement being detached"
                        );
                        *parent_info.get_tail_mut() = prev;
                    }
                }

                Ok(())
            }
        }
    };
}

impl_detach!(StatementRef);
impl_detach!(Block);
