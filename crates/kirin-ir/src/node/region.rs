use crate::arena::{GetInfo, Id, Item};
use crate::{Dialect, identifier};

use super::block::Block;
use super::linked_list::LinkedList;
use super::stmt::Statement;

identifier! {
    /// A unique identifier for a region.
    struct Region
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RegionInfo<L: Dialect> {
    pub(crate) id: Region,
    pub(crate) parent: Option<Statement>,
    pub(crate) blocks: LinkedList<Block>,
    _marker: std::marker::PhantomData<L>,
}

#[bon::bon]
impl<L: Dialect> RegionInfo<L> {
    #[builder(finish_fn = new)]
    pub fn new(
        /// The unique identifier for this region.
        id: Region,
        /// The parent statement of this region, if any.
        parent: Option<Statement>,
        /// The blocks contained in this region.
        blocks: LinkedList<Block>,
    ) -> Self {
        Self {
            id,
            parent,
            blocks,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<L: Dialect> GetInfo<L> for Region {
    type Info = Item<RegionInfo<L>>;

    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info> {
        context.regions.get(*self)
    }

    fn get_info_mut<'a>(&self, context: &'a mut crate::Context<L>) -> Option<&'a mut Self::Info> {
        context.regions.get_mut(*self)
    }
}

impl Region {
    pub fn blocks<'a, L: Dialect>(&self, context: &'a crate::Context<L>) -> BlockIter<'a, L> {
        let info = self.expect_info(context);
        BlockIter {
            current: info.blocks.head,
            len: info.blocks.len,
            context,
        }
    }
}

pub struct BlockIter<'a, L: Dialect> {
    current: Option<Block>,
    len: usize,
    context: &'a crate::Context<L>,
}

impl<'a, L: Dialect> Iterator for BlockIter<'a, L> {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current {
            let current_info = current.expect_info(self.context);
            self.current = current_info.node.next;
            self.len -= 1;
            Some(current)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, L: Dialect> ExactSizeIterator for BlockIter<'a, L> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, L: Dialect> DoubleEndedIterator for BlockIter<'a, L> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(tail) = self.current {
            let tail_info = tail.expect_info(self.context);
            self.current = tail_info.node.prev;
            self.len -= 1;
            Some(tail)
        } else {
            None
        }
    }
}
