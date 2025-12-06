use crate::{
    Dialect,
    arena::{GetInfo, Id, Item},
    identifier,
    node::region::Region,
};

use super::{
    linked_list::{LinkedList, LinkedListNode},
    ssa::BlockArgument,
    stmt::Statement,
};

identifier! {
    /// A unique identifier for a block, used in statement declarations
    /// means the statement owns a block.
    struct Block
}

identifier! {
    /// A unique identifier for a successor block, if used in statement
    /// declarations means the statement may transfer control to the
    /// successor block.
    struct Successor
}

impl From<Successor> for Block {
    fn from(succ: Successor) -> Self {
        Block(succ.0)
    }
}

impl From<Block> for Successor {
    fn from(block: Block) -> Self {
        Successor(block.0)
    }
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "^{}", self.0.raw())
    }
}

impl std::fmt::Display for Successor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "^{}", self.0.raw())
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockInfo<L: Dialect> {
    pub parent: Option<Region>,
    pub node: LinkedListNode<Block>,
    pub arguments: Vec<BlockArgument>,
    pub statements: LinkedList<Statement>,
    pub terminator: Option<Statement>,
    _marker: std::marker::PhantomData<L>,
}

#[bon::bon]
impl<L: Dialect> BlockInfo<L> {
    #[builder(finish_fn = new)]
    pub(crate) fn new(
        /// The parent region of this block.
        parent: Option<Region>,
        /// The linked list node for this block.
        node: LinkedListNode<Block>,
        /// The arguments of this block.
        arguments: Vec<BlockArgument>,
        /// The statements contained in this block.
        statements: Option<LinkedList<Statement>>,
        /// The terminator statement of this block, if any.
        terminator: Option<Statement>,
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

impl<L: Dialect> GetInfo<L> for Block {
    type Info = Item<BlockInfo<L>>;

    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info> {
        context.blocks.get(*self)
    }

    fn get_info_mut<'a>(&self, context: &'a mut crate::Context<L>) -> Option<&'a mut Self::Info> {
        context.blocks.get_mut(*self)
    }
}

impl Block {
    pub fn statements<'a, L: Dialect>(
        &self,
        context: &'a crate::Context<L>,
    ) -> StatementIter<'a, L> {
        let info = self.expect_info(context);
        StatementIter {
            current: info.statements.head,
            len: info.statements.len,
            context,
        }
    }
}

pub struct StatementIter<'a, L: Dialect> {
    current: Option<Statement>,
    len: usize,
    context: &'a crate::Context<L>,
}

impl<'a, L: Dialect> Iterator for StatementIter<'a, L> {
    type Item = Statement;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current {
            let info = current.expect_info(self.context);
            self.current = info.node.next;
            self.len -= 1;
            Some(current)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, L: Dialect> ExactSizeIterator for StatementIter<'a, L> {
    fn len(&self) -> usize {
        self.len
    }
}
