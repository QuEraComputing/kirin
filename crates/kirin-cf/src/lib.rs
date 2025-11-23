use kirin::ir::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Statement)]
#[kirin(terminator)]
pub enum ControlFlow {
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

impl ControlFlow {
    pub fn branch<L: Language + From<ControlFlow>>(arena: &mut Arena<L>, target: Block) -> StatementId {
        let instr = ControlFlow::Branch { target };
        arena.statement().definition(instr).new()
    }

    pub fn conditional_branch<L: Language + From<ControlFlow>>(
        arena: &mut Arena<L>,
        condition: SSAValue,
        true_target: Block,
        false_target: Block,
    ) -> StatementId {
        let instr = ControlFlow::ConditionalBranch {
            condition,
            true_target,
            false_target,
        };
        arena.statement().definition(instr).new()
    }

    pub fn return_instr<L: Language + From<ControlFlow>>(arena: &mut Arena<L>, value: SSAValue) -> StatementId {
        let instr = ControlFlow::Return(value);
        arena.statement().definition(instr).new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cf() {
        let inst = ControlFlow::Return(TestSSAValue(0).into());
        for succ in inst.successors() {
            println!("Successor: {:?}", succ);
        }
    }
}
