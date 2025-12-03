use kirin::ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Statement)]
pub enum StructuredControlFlow {
    If {
        condition: SSAValue,
        then_block: Block,
        else_block: Block,
    },
    Loop {
        body_block: Block,
        exit_block: Block,
    },
}

impl StructuredControlFlow {
    pub fn op_if<L>(
        context: &mut Context<L>,
        condition: SSAValue,
        then_block: Block,
        else_block: Block,
    ) -> StatementId
    where
        L: Language + From<StructuredControlFlow>,
    {
        let instr = StructuredControlFlow::If {
            condition,
            then_block,
            else_block,
        };
        context.statement().definition(instr).new()
    }
}
