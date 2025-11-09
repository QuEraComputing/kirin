use crate::Language;

use super::block::Block;
use super::linked_list::LinkedList;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Region(usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RegionInfo<L: Language> {
    pub blocks: LinkedList<Block>,
    _marker: std::marker::PhantomData<L>,
}

impl Region {
    pub fn id(&self) -> usize {
        self.0
    }
}
