use crate::{
    Dialect, Symbol,
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

impl Successor {
    /// Extracts the underlying block this successor targets.
    pub fn target(self) -> Block {
        Block(self.0)
    }

    /// Creates a successor from a block identifier.
    pub fn from_block(block: Block) -> Self {
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
    pub name: Option<Symbol>,
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
        /// The name of this block.
        name: Option<Symbol>,
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
            name,
            node,
            arguments,
            statements: statements.unwrap_or_default(),
            terminator,
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns the name of this block, if any.
    pub fn name(&self) -> Option<Symbol> {
        self.name
    }
}

impl<L: Dialect> GetInfo<L> for Block {
    type Info = Item<BlockInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.blocks.get(*self)
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.blocks.get_mut(*self)
    }
}

impl<L: Dialect> GetInfo<L> for Successor {
    type Info = Item<BlockInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.blocks.get(self.target())
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.blocks.get_mut(self.target())
    }
}

impl Block {
    pub fn statements<'a, L: Dialect>(
        &self,
        stage: &'a crate::StageInfo<L>,
    ) -> StatementIter<'a, L> {
        let info = self.expect_info(stage);
        StatementIter {
            head: info.statements.head,
            tail: info.statements.tail,
            len: info.statements.len,
            stage,
        }
    }

    pub fn terminator<L: Dialect>(&self, stage: &crate::StageInfo<L>) -> Option<Statement> {
        let info = self.expect_info(stage);
        info.terminator
    }

    /// Returns the first statement in this block.
    ///
    /// This is the head of the statements linked list, or the terminator
    /// if the linked list is empty (i.e. the block contains only a
    /// terminator).
    pub fn first_statement<L: Dialect>(&self, stage: &crate::StageInfo<L>) -> Option<Statement> {
        let info = self.expect_info(stage);
        if let Some(&head) = info.statements.head() {
            Some(head)
        } else {
            info.terminator
        }
    }

    /// Returns the last statement in this block.
    ///
    /// The terminator *is* the last statement — the `terminator` field in
    /// [`BlockInfo`] is a cached pointer to it, not a separate statement.
    /// [`Block::statements`] iterates only the non-terminator prefix of
    /// the linked list. This method returns the terminator if present,
    /// otherwise the tail of the statements linked list.
    pub fn last_statement<L: Dialect>(&self, stage: &crate::StageInfo<L>) -> Option<Statement> {
        let info = self.expect_info(stage);
        info.terminator.or_else(|| info.statements.tail().copied())
    }
}

pub struct StatementIter<'a, L: Dialect> {
    head: Option<Statement>,
    tail: Option<Statement>,
    len: usize,
    stage: &'a crate::StageInfo<L>,
}

impl<'a, L: Dialect> Iterator for StatementIter<'a, L> {
    type Item = Statement;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(head) = self.head {
            let info = head.expect_info(self.stage);
            self.len -= 1;
            if self.len == 0 {
                self.head = None;
                self.tail = None;
            } else {
                self.head = info.node.next;
            }
            Some(head)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, L: Dialect> DoubleEndedIterator for StatementIter<'a, L> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(tail) = self.tail {
            let info = tail.expect_info(self.stage);
            self.len -= 1;
            if self.len == 0 {
                self.head = None;
                self.tail = None;
            } else {
                self.tail = info.node.prev;
            }
            Some(tail)
        } else {
            None
        }
    }
}

impl<'a, L: Dialect> ExactSizeIterator for StatementIter<'a, L> {
    fn len(&self) -> usize {
        self.len
    }
}
