use kirin_ir::{Block, Dialect, StageInfo, Statement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExecutionCursor {
    block: Block,
    statement: Option<Statement>,
}

impl ExecutionCursor {
    pub(crate) fn entry<L: Dialect>(stage: &StageInfo<L>, block: Block) -> Self {
        Self {
            block,
            statement: block.first_statement(stage),
        }
    }

    pub(crate) const fn statement(&self) -> Option<Statement> {
        self.statement
    }

    pub(crate) fn jump_to<L: Dialect>(&mut self, stage: &StageInfo<L>, block: Block) {
        self.block = block;
        self.statement = block.first_statement(stage);
    }

    pub(crate) fn advance<L: Dialect>(&mut self, stage: &StageInfo<L>) {
        let Some(current) = self.statement else {
            return;
        };

        self.statement = (*current.next(stage)).or_else(|| {
            let terminator = self.block.terminator(stage);
            if terminator == Some(current) {
                None
            } else {
                terminator
            }
        });
    }
}
