use crate::{Language, node::region::Region};

use super::{linked_list::{Node, LinkedList}, ssa::BlockArgument, stmt::Statement};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Block(usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockInfo<L: Language> {
    pub parent: Option<Region>,
    pub node: Node<Block>,
    pub arguments: Vec<BlockArgument>,
    pub statements: LinkedList<Statement>,
    pub terminator: Option<Statement>,
    _marker: std::marker::PhantomData<L>,
}

impl Block {
    pub fn id(&self) -> usize {
        self.0
    }
}
