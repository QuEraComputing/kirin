//! Linking and unlinking the IR's two doubly linked lists: a block's
//! statement chain and a CFG's block list. Each operation is written once,
//! for any [`ListItem`].

use crate::arena::{GetInfo, GetInfoMut};
use crate::node::linked_list::{LinkedList, LinkedListNode};
use crate::rewrite::rewriter::Rewriter;
use crate::{Block, BlockParent, CFG, Dialect, Statement, StatementParent};

/// An item of one of the IR's linked lists, kept in a list its `Owner` holds:
/// a [`Statement`] in its block's statement chain, or a [`Block`] in its
/// CFG's block list.
pub(super) trait ListItem<L: Dialect>: GetInfoMut<L> + Copy + PartialEq {
    /// What holds the list.
    type Owner: GetInfoMut<L> + Copy;

    /// The item's links to its neighbours.
    fn node(info: &mut Self::Info) -> &mut LinkedListNode<Self>;

    /// Make `owner` the item's parent.
    fn set_owner(info: &mut Self::Info, owner: Self::Owner);

    /// The list `owner` holds.
    fn list(info: &mut <Self::Owner as GetInfo<L>>::Info) -> &mut LinkedList<Self>;
}

impl<L: Dialect> ListItem<L> for Statement {
    type Owner = Block;

    fn node(info: &mut Self::Info) -> &mut LinkedListNode<Self> {
        &mut info.node
    }

    fn set_owner(info: &mut Self::Info, owner: Block) {
        info.parent = Some(StatementParent::Block(owner));
    }

    fn list(info: &mut <Block as GetInfo<L>>::Info) -> &mut LinkedList<Self> {
        &mut info.statements
    }
}

impl<L: Dialect> ListItem<L> for Block {
    type Owner = CFG;

    fn node(info: &mut Self::Info) -> &mut LinkedListNode<Self> {
        &mut info.node
    }

    fn set_owner(info: &mut Self::Info, owner: CFG) {
        info.parent = Some(BlockParent::CFG(owner));
    }

    fn list(info: &mut <CFG as GetInfo<L>>::Info) -> &mut LinkedList<Self> {
        &mut info.blocks
    }
}

// Items and owners that are no longer live are skipped rather than
// panicking on, as the rewriter's edits check liveness before linking.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Link `chain`, a chain of items not in any list, into `owner`'s list
    /// between `prev` and `next` (the list's head or tail when one is
    /// `None`), making `owner` the parent of each item. For a single item,
    /// pass [`LinkedList::single`].
    pub(super) fn link<T: ListItem<L>>(
        &mut self,
        chain: LinkedList<T>,
        owner: T::Owner,
        prev: Option<T>,
        next: Option<T>,
    ) {
        let (Some(head), Some(tail)) = (chain.head, chain.tail) else {
            return;
        };
        // Stop at `tail`: a single item may still carry links from before
        // it was unlinked.
        let mut current = Some(head);
        while let Some(item) = current
            && let Some(info) = item.get_info_mut(self.stage)
        {
            T::set_owner(info, owner);
            current = if item == tail {
                None
            } else {
                T::node(info).next
            };
        }
        if let Some(info) = head.get_info_mut(self.stage) {
            T::node(info).prev = prev;
        }
        if let Some(info) = tail.get_info_mut(self.stage) {
            T::node(info).next = next;
        }
        match prev {
            Some(prev) => {
                if let Some(info) = prev.get_info_mut(self.stage) {
                    T::node(info).next = Some(head);
                }
            }
            None => {
                if let Some(info) = owner.get_info_mut(self.stage) {
                    T::list(info).head = Some(head);
                }
            }
        }
        match next {
            Some(next) => {
                if let Some(info) = next.get_info_mut(self.stage) {
                    T::node(info).prev = Some(tail);
                }
            }
            None => {
                if let Some(info) = owner.get_info_mut(self.stage) {
                    T::list(info).tail = Some(tail);
                }
            }
        }
        if let Some(info) = owner.get_info_mut(self.stage) {
            T::list(info).len += chain.len;
        }
    }

    /// Unlink `item` from `owner`'s list, joining its neighbours (or moving
    /// the list's head or tail when it is at an end) and clearing its own
    /// links. Its parent is left for the caller.
    pub(super) fn unlink<T: ListItem<L>>(&mut self, item: T, owner: T::Owner) {
        let Some(info) = item.get_info_mut(self.stage) else {
            debug_assert!(false, "unlinking an item that is not live");
            return;
        };
        let node = T::node(info);
        let (prev, next) = (node.prev, node.next);
        node.prev = None;
        node.next = None;
        match prev {
            Some(prev) => {
                if let Some(info) = prev.get_info_mut(self.stage) {
                    T::node(info).next = next;
                }
            }
            None => {
                if let Some(info) = owner.get_info_mut(self.stage) {
                    T::list(info).head = next;
                }
            }
        }
        match next {
            Some(next) => {
                if let Some(info) = next.get_info_mut(self.stage) {
                    T::node(info).prev = prev;
                }
            }
            None => {
                if let Some(info) = owner.get_info_mut(self.stage) {
                    T::list(info).tail = prev;
                }
            }
        }
        if let Some(info) = owner.get_info_mut(self.stage) {
            let list = T::list(info);
            debug_assert!(list.len > 0, "unlinking from an empty list");
            list.len = list.len.saturating_sub(1);
        }
    }

    /// Cut `at` and every item after it off `owner`'s list, returning them as
    /// a chain, ready for [`link`](Self::link). Their parent is left for the
    /// caller.
    pub(super) fn split_off<T: ListItem<L>>(&mut self, owner: T::Owner, at: T) -> LinkedList<T> {
        let mut len = 0;
        let mut current = Some(at);
        while let Some(item) = current {
            len += 1;
            current = item
                .get_info_mut(self.stage)
                .and_then(|info| T::node(info).next);
        }
        let prev = match at.get_info_mut(self.stage) {
            Some(info) => T::node(info).prev.take(),
            None => return LinkedList::new(),
        };
        if let Some(prev) = prev
            && let Some(info) = prev.get_info_mut(self.stage)
        {
            T::node(info).next = None;
        }
        let Some(info) = owner.get_info_mut(self.stage) else {
            return LinkedList::new();
        };
        let list = T::list(info);
        let tail = list.tail;
        if prev.is_none() {
            list.head = None;
        }
        list.tail = prev;
        list.len = list.len.saturating_sub(len);
        LinkedList {
            head: Some(at),
            tail,
            len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::dialect::{TestLang, new_stage};
    use crate::{GetInfo, StageInfo};

    /// A block holding `count` `nop`s, and the `nop`s in order.
    fn block_of_nops(count: usize) -> (StageInfo<TestLang>, Block, Vec<Statement>) {
        let mut stage = new_stage();
        let nops: Vec<Statement> = (0..count)
            .map(|_| stage.statement().definition(TestLang::Nop).new())
            .collect();
        let block = nops
            .iter()
            .fold(stage.block(), |block, &nop| block.stmt(nop))
            .new();
        (stage.finalize().unwrap(), block, nops)
    }

    /// `block`'s statements in order, after checking that walking the list
    /// backwards gives the same order, that its length is right, and that
    /// every statement names `block` as its parent.
    fn statements(stage: &StageInfo<TestLang>, block: Block) -> Vec<Statement> {
        let list = block.expect_info(stage).statements;
        let mut forward = Vec::new();
        let mut current = list.head;
        while let Some(stmt) = current {
            forward.push(stmt);
            current = stmt.expect_info(stage).node.next;
        }
        let mut backward = Vec::new();
        let mut current = list.tail;
        while let Some(stmt) = current {
            backward.push(stmt);
            current = stmt.expect_info(stage).node.prev;
        }
        backward.reverse();
        assert_eq!(forward, backward);
        assert_eq!(list.len, forward.len());
        for stmt in &forward {
            assert_eq!(
                stmt.expect_info(stage).parent,
                Some(StatementParent::Block(block))
            );
        }
        forward
    }

    #[test]
    fn link_places_a_single_item_at_the_head_middle_and_tail() {
        let (mut stage, block, nops) = block_of_nops(4);
        let [a, b, c, d] = nops[..] else {
            unreachable!()
        };
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter.unlink(a, block);
        rewriter.unlink(c, block);
        rewriter.unlink(d, block);
        assert_eq!(statements(rewriter.stage(), block), [b]);

        rewriter.link(LinkedList::single(d), block, Some(b), None);
        rewriter.link(LinkedList::single(a), block, None, Some(b));
        rewriter.link(LinkedList::single(c), block, Some(b), Some(d));
        assert_eq!(statements(rewriter.stage(), block), [a, b, c, d]);
    }

    #[test]
    fn unlinking_the_only_item_empties_the_list() {
        let (mut stage, block, nops) = block_of_nops(1);
        let mut rewriter = Rewriter::new(&mut stage);
        rewriter.unlink(nops[0], block);
        assert_eq!(statements(rewriter.stage(), block), []);
        let list = block.expect_info(rewriter.stage()).statements;
        assert_eq!((list.head, list.tail), (None, None));
        let node = nops[0].expect_info(rewriter.stage()).node;
        assert_eq!((node.prev, node.next), (None, None));
    }

    #[test]
    fn split_off_in_the_middle_keeps_the_front_and_returns_the_rest() {
        let (mut stage, block, nops) = block_of_nops(4);
        let [a, b, c, d] = nops[..] else {
            unreachable!()
        };
        let mut rewriter = Rewriter::new(&mut stage);
        let rest = rewriter.split_off(block, c);
        assert_eq!(statements(rewriter.stage(), block), [a, b]);
        assert_eq!((rest.head, rest.tail, rest.len), (Some(c), Some(d), 2));

        // Linked back at the end, the block is as it was.
        rewriter.link(rest, block, Some(b), None);
        assert_eq!(statements(rewriter.stage(), block), [a, b, c, d]);
    }

    #[test]
    fn split_off_at_the_head_takes_the_whole_list() {
        let (mut stage, block, nops) = block_of_nops(3);
        let mut rewriter = Rewriter::new(&mut stage);
        let all = rewriter.split_off(block, nops[0]);
        assert_eq!(statements(rewriter.stage(), block), []);
        assert_eq!(
            (all.head, all.tail, all.len),
            (Some(nops[0]), Some(nops[2]), 3)
        );

        // Linked into an empty list, they become the whole list again.
        rewriter.link(all, block, None, None);
        assert_eq!(statements(rewriter.stage(), block), nops);
    }
}
