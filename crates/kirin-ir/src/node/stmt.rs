use crate::{IRContext, language::Language, node::linked_list::Node};

use super::block::Block;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Statement(usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementInfo<L: Language> {
    pub(crate) node: Node<Statement>,
    pub(crate) parent: Option<Block>,
    pub(crate) info: L,
}

impl Statement {
    pub fn id(&self) -> usize {
        self.0
    }
}
