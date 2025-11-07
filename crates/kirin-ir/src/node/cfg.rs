use super::block::Block;
use super::linked_list::LinkedList;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CFG(LinkedList<Block>);

impl CFG {
    /// Creates a new, empty CFG.
    pub fn new() -> Self {
        CFG(LinkedList::new())
    }

    /// Returns the entry block of the CFG, if it exists.
    /// `None` if the CFG is empty.
    pub fn entry(&self) -> Option<&Block> {
        self.0.head()
    }

    pub fn blocks(&self) -> impl Iterator<Item = Block> {
        self.0.iter()
    }
}
