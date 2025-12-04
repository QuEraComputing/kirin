use crate::arena::{GetInfo, Id, Item};
use crate::{Dialect, identifier};

use super::block::Block;
use super::linked_list::LinkedList;
use super::stmt::StatementId;

identifier! {
    /// A unique identifier for a region.
    struct Region
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RegionInfo<L: Dialect> {
    pub(crate) id: Region,
    pub(crate) parent: Option<StatementId>,
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
        parent: Option<StatementId>,
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

    fn get_info<'a>(
        &self,
        context: &'a crate::Context<L>,
    ) -> Option<&'a Self::Info> {
        context.regions.get(*self)
    }

    fn get_info_mut<'a>(
        &self,
        context: &'a mut crate::Context<L>,
    ) -> Option<&'a mut Self::Info> {
        context.regions.get_mut(*self)
    }
}
