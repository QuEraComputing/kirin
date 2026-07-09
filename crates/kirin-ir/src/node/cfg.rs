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

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.regions.get(*self)
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.regions.get_mut(*self)
    }
}

impl Region {
    pub fn blocks<'a, L: Dialect>(&self, stage: &'a crate::StageInfo<L>) -> BlockIter<'a, L> {
        let info = self.expect_info(stage);
        BlockIter {
            head: info.blocks.head,
            tail: info.blocks.tail,
            len: info.blocks.len,
            stage,
        }
    }
}

pub struct BlockIter<'a, L: Dialect> {
    head: Option<Block>,
    tail: Option<Block>,
    len: usize,
    stage: &'a crate::StageInfo<L>,
}

impl<'a, L: Dialect> Iterator for BlockIter<'a, L> {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(head) = self.head {
            let head_info = head.expect_info(self.stage);
            self.len -= 1;
            if self.len == 0 {
                self.head = None;
                self.tail = None;
            } else {
                self.head = head_info.node.next;
            }
            Some(head)
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
        if let Some(tail) = self.tail {
            let tail_info = tail.expect_info(self.stage);
            self.len -= 1;
            if self.len == 0 {
                self.head = None;
                self.tail = None;
            } else {
                self.tail = tail_info.node.prev;
            }
            Some(tail)
        } else {
            None
        }
    }
}
