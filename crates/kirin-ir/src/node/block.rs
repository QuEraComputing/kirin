use crate::{
    Language,
    arena::{GetInfo, Id, Item},
    identifier,
    node::region::Region,
};

use super::{
    linked_list::{LinkedList, LinkedListNode},
    ssa::BlockArgument,
    stmt::StatementId,
};

identifier! {
    /// A unique identifier for a block.
    struct Block
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockInfo<L: Language> {
    pub parent: Option<Region>,
    pub node: LinkedListNode<Block>,
    pub arguments: Vec<BlockArgument>,
    pub statements: LinkedList<StatementId>,
    pub terminator: Option<StatementId>,
    _marker: std::marker::PhantomData<L>,
}

#[bon::bon]
impl<L: Language> BlockInfo<L> {
    #[builder(finish_fn = new)]
    pub(crate) fn new(
        /// The parent region of this block.
        parent: Option<Region>,
        /// The linked list node for this block.
        node: LinkedListNode<Block>,
        /// The arguments of this block.
        arguments: Vec<BlockArgument>,
        /// The statements contained in this block.
        statements: Option<LinkedList<StatementId>>,
        /// The terminator statement of this block, if any.
        terminator: Option<StatementId>,
    ) -> Self {
        Self {
            parent,
            node,
            arguments,
            statements: statements.unwrap_or_default(),
            terminator,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<L: Language> GetInfo<L> for Block {
    type Info = Item<BlockInfo<L>>;

    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info> {
        context.blocks.get(*self)
    }

    fn get_info_mut<'a>(&self, context: &'a mut crate::Context<L>) -> Option<&'a mut Self::Info> {
        context.blocks.get_mut(*self)
    }
}
