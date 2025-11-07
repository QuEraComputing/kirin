use crate::ir::{SSAValue, linked_list::LinkedList, stmt::Statement};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Block(usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockInfo {
    pub id: Block,
    pub arguments: Vec<SSAValue>,
    pub statements: LinkedList<Statement>,
    pub terminator: Option<Statement>,
}
