use crate::ir::{Block, Instruction, ResultValue, SSAValue};

pub enum SCFInstruction {
    If {
        condition: SSAValue,
        then_block: Block,
        else_block: Block,
        results: Vec<ResultValue>,
    },
    For {
        lower_bound: SSAValue,
        upper_bound: SSAValue,
        step: SSAValue,
        body_block: Block,
        results: Vec<ResultValue>,
    },
}

impl Instruction for SCFInstruction {
    type ResultIterator = std::vec::IntoIter<ResultValue>;

    fn results(&self) -> Self::ResultIterator {
        match self {
            SCFInstruction::If { results, .. } => results.clone().into_iter(),
            SCFInstruction::For { results, .. } => results.clone().into_iter(),
        }
    }
}
