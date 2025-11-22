use crate::Language;

use super::block::Block;
use super::linked_list::LinkedList;
use super::stmt::StatementId;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Region(pub(crate) usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RegionInfo<L: Language> {
    pub(crate) id: Region,
    pub(crate) parent: Option<StatementId>,
    pub(crate) blocks: LinkedList<Block>,
    _marker: std::marker::PhantomData<L>,
}

#[bon::bon]
impl<L: Language> RegionInfo<L> {
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

impl Region {
    pub fn id(&self) -> usize {
        self.0
    }
}
