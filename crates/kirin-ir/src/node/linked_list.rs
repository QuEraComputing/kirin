use super::block::Block;
use super::stmt::StatementId;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LinkedListNode<Ptr: Copy + PartialEq> {
    pub ptr: Ptr,
    pub next: Option<Ptr>,
    pub prev: Option<Ptr>,
}

impl<Ptr: Copy + PartialEq> LinkedListNode<Ptr> {
    pub fn new(ptr: Ptr) -> Self {
        LinkedListNode {
            ptr,
            next: None,
            prev: None,
        }
    }
}

impl From<LinkedListNode<StatementId>> for StatementId {
    fn from(value: LinkedListNode<StatementId>) -> Self {
        value.ptr
    }
}

impl From<LinkedListNode<Block>> for Block {
    fn from(value: LinkedListNode<Block>) -> Self {
        value.ptr
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct LinkedList<Ptr: Copy + PartialEq> {
    pub(crate) head: Option<Ptr>,
    pub(crate) tail: Option<Ptr>,
    pub(crate) len: usize,
}

impl<Ptr: Copy + PartialEq> LinkedList<Ptr> {
    pub fn new() -> Self {
        LinkedList {
            head: None,
            tail: None,
            len: 0,
        }
    }

    pub fn head(&self) -> Option<&Ptr> {
        self.head.as_ref()
    }

    pub fn tail(&self) -> Option<&Ptr> {
        self.tail.as_ref()
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl<Ptr: Copy + PartialEq> Default for LinkedList<Ptr> {
    fn default() -> Self {
        Self::new()
    }
}
