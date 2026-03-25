use kirin_ir::{Block, Dialect, StageInfo, Statement};

/// Linear block-local shell cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockCursor {
    block: Block,
    current: Option<Statement>,
}

impl BlockCursor {
    pub(crate) fn new<L: Dialect>(stage: &StageInfo<L>, block: Block) -> Self {
        Self {
            block,
            current: block.first_statement(stage),
        }
    }

    pub(crate) fn at_statement(block: Block, statement: Statement) -> Self {
        Self {
            block,
            current: Some(statement),
        }
    }

    pub(crate) fn exhausted(block: Block) -> Self {
        Self {
            block,
            current: None,
        }
    }

    pub(crate) fn block(&self) -> Block {
        self.block
    }

    pub(crate) fn current(&self) -> Option<Statement> {
        self.current
    }

    pub(crate) fn advance<L: Dialect>(&mut self, stage: &StageInfo<L>) {
        let Some(current) = self.current else {
            return;
        };

        self.current = if Some(current) == self.block.last_statement(stage) {
            None
        } else {
            (*current.next(stage)).or_else(|| self.block.terminator(stage))
        };
    }
}
