use crate::{language::Language, node::linked_list::LinkedListNode};

use super::block::Block;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementRef(usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementInfo<L: Language> {
    pub(crate) node: LinkedListNode<StatementRef>,
    pub(crate) parent: Option<Block>,
    pub(crate) info: L,
}

impl StatementRef {
    pub fn id(&self) -> usize {
        self.0
    }
}
