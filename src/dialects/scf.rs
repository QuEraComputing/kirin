use kirin_ir::*;

use crate::ir::{Block, HasArguments, ResultValue, SSAValue};

#[derive(
    Clone,
    Hash,
    PartialEq,
    Eq,
    Debug,
    HasArguments,
    IsPure,
    IsTerminator,
    IsConstant,
    HasRegions,
    HasSuccessors,
)]
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

impl<'a> HasResults<'a> for SCFInstruction {
    type Iter = std::slice::Iter<'a, ResultValue>;
    fn results(&'a self) -> Self::Iter {
        match self {
            SCFInstruction::If { results, .. } => results.iter(),
            SCFInstruction::For { results, .. } => results.iter(),
        }
    }
}
impl Statement<'_> for SCFInstruction {}
