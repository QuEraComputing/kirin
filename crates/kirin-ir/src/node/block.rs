use crate::{Language, node::region::Region};

use super::{
    linked_list::{LinkedList, LinkedListNode},
    ssa::BlockArgument,
    stmt::StatementId,
};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Block(pub(crate) usize);

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

impl Block {
    pub fn id(&self) -> usize {
        self.0
    }

    // pub fn insert_after<L: Language>(&self, arena: &mut crate::Arena<L>, stmt: StatementId) {
    //     let block_info = arena
    //         .get_block_mut(*self)
    //         .expect("Invalid Block in given arena");
    //     block_info.statements
    // }
}
