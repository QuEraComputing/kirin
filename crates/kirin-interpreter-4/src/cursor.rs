use kirin_ir::{Block, Dialect, StageInfo, Statement};

/// Linear cursor over statements in a single block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockCursor {
    block: Block,
    current: Option<Statement>,
}

impl BlockCursor {
    pub fn new<L: Dialect>(stage: &StageInfo<L>, block: Block) -> Self {
        Self {
            block,
            current: block.first_statement(stage),
        }
    }

    pub fn block(&self) -> Block {
        self.block
    }

    pub fn current(&self) -> Option<Statement> {
        self.current
    }

    pub fn is_exhausted(&self) -> bool {
        self.current.is_none()
    }

    pub fn advance<L: Dialect>(&mut self, stage: &StageInfo<L>) {
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
