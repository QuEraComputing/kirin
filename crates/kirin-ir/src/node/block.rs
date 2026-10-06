use smallvec::SmallVec;

use crate::{
    Dialect, Symbol,
    arena::{GetInfo, GetInfoMut, Id, Item},
    identifier,
};

use super::{
    cfg::CFG,
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

/// The immediate structural owner of a block.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BlockParent {
    /// The block belongs to a block-list control-flow body.
    CFG(CFG),
    /// The block is a single-block body owned directly by a statement.
    Statement(Statement),
}

/// Produces CFG(id) instead of CFG(CFG(id))
impl std::fmt::Debug for BlockParent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockParent::CFG(cfg) => write!(f, "{cfg:?}"),
            BlockParent::Statement(stmt) => write!(f, "{stmt:?}"),
        }
    }
}

/// What a block holds.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockInfo<L: Dialect> {
    pub(crate) parent: Option<BlockParent>,
    pub(crate) name: Option<Symbol>,
    pub(crate) node: LinkedListNode<Block>,
    pub(crate) arguments: Vec<BlockArgument>,
    /// Reverse control-flow index: blocks whose terminators may transfer
    /// control to this block.
    ///
    /// Inline capacity 4: a straight-line block has one predecessor, an
    /// if-merge or loop header has two, and a small switch join or a loop
    /// with a couple of `break`s stays under four. Wider joins spill to the
    /// heap rather than making every block pay for the worst case.
    pub(crate) predecessors: SmallVec<[Block; 4]>,
    pub(crate) statements: LinkedList<Statement>,
    pub(crate) terminator: Option<Statement>,
    _marker: std::marker::PhantomData<L>,
}

#[bon::bon]
impl<L: Dialect> BlockInfo<L> {
    #[builder(finish_fn = new)]
    pub(crate) fn new(
        /// The immediate CFG or statement parent of this block.
        parent: Option<BlockParent>,
        /// The name of this block.
        name: Option<Symbol>,
        /// The linked list node for this block.
        node: LinkedListNode<Block>,
        /// The arguments of this block.
        arguments: Vec<BlockArgument>,
        /// The predecessor blocks in the reverse control-flow index.
        predecessors: SmallVec<[Block; 4]>,
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
            predecessors,
            statements: statements.unwrap_or_default(),
            terminator,
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns the name of this block, if any.
    pub fn name(&self) -> Option<Symbol> {
        self.name
    }

    /// The CFG or statement that structurally owns this block.
    pub fn parent(&self) -> Option<BlockParent> {
        self.parent
    }

    /// This block's position among its siblings.
    pub fn node(&self) -> &LinkedListNode<Block> {
        &self.node
    }

    /// The values this block takes on entry.
    pub fn arguments(&self) -> &[BlockArgument] {
        &self.arguments
    }

    /// The blocks whose terminators may transfer control here.
    ///
    /// A derived mirror of those terminators' successor operands, so it is
    /// readable but not writable from outside the crate.
    pub fn predecessors(&self) -> &[Block] {
        &self.predecessors
    }

    /// The head/tail/length summary of this block's non-terminator statements.
    ///
    /// A derived mirror of the `prev`/`next` links on those statements. To walk
    /// the body, prefer [`Block::statements`], which follows the links.
    pub fn statements(&self) -> &LinkedList<Statement> {
        &self.statements
    }

    /// The cached pointer to this block's terminator.
    ///
    /// A derived mirror of which member statement is a terminator, not a
    /// statement separate from the body.
    pub fn terminator(&self) -> Option<Statement> {
        self.terminator
    }
}

impl<L: Dialect> GetInfo<L> for Block {
    type Info = Item<BlockInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.blocks.get(*self)
    }
}

impl<L: Dialect> GetInfoMut<L> for Block {
    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.blocks.get_mut(*self)
    }
}

impl<L: Dialect> GetInfo<L> for Successor {
    type Info = Item<BlockInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.blocks.get(self.target())
    }
}

impl<L: Dialect> GetInfoMut<L> for Successor {
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
