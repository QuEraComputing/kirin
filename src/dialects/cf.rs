use crate::ir::{Block, Instruction, SSAValue};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Instruction)]
#[kirin(is_terminator = true)]
pub enum ControlFlowInstruction {
    Branch {
        target: Block,
    },
    ConditionalBranch {
        condition: SSAValue,
        true_target: Block,
        false_target: Block,
    },
    Return(SSAValue),
}
