use crate::ir::{Block, Instruction, SSAValue};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
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

impl Instruction for ControlFlowInstruction {
    type ResultIterator = std::iter::Empty<crate::ir::ResultValue>;

    fn results(&self) -> Self::ResultIterator {
        std::iter::empty()
    }

    fn is_terminator(&self) -> bool {
        true
    }

    fn successors(&self) -> impl Iterator<Item = Block> {
        CFSuccessorsIterator::new(self.clone())
    }
}

pub struct CFSuccessorsIterator {
    instruction: ControlFlowInstruction,
    state: u8,
}

impl CFSuccessorsIterator {
    pub fn new(instruction: ControlFlowInstruction) -> Self {
        Self {
            instruction,
            state: 0,
        }
    }
}

impl Iterator for CFSuccessorsIterator {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        match self.instruction {
            ControlFlowInstruction::Branch { target } => {
                if self.state == 0 {
                    self.state += 1;
                    Some(target)
                } else {
                    None
                }
            }
            ControlFlowInstruction::ConditionalBranch {
                true_target,
                false_target,
                ..
            } => match self.state {
                0 => {
                    self.state += 1;
                    Some(true_target)
                }
                1 => {
                    self.state += 1;
                    Some(false_target)
                }
                _ => None,
            },
            ControlFlowInstruction::Return(_) => None,
        }
    }
}
