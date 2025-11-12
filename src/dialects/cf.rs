use crate::ir::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Statement)]
#[kirin(terminator)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cf() {
        let inst = ControlFlowInstruction::Return(TestSSAValue(0).into());
        for succ in inst.successors() {
            println!("Successor: {:?}", succ);
        }
    }
}