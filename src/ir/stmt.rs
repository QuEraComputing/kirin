use crate::ir::block::Block;
use crate::language::Language;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Statement(usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementInfo<L: Language> {
    id: Statement,
    info: L,
    successors: Vec<Block>,
}

/// minimal information about an instruction.
pub trait Instruction {
    type ResultIterator: Iterator<Item = crate::ir::ResultValue>;
    fn results(&self) -> Self::ResultIterator;

    fn is_terminator(&self) -> bool {
        false
    }

    fn successors(&self) -> impl Iterator<Item = Block> {
        std::iter::empty()
    }
}
