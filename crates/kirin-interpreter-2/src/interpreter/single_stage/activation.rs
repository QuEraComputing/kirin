use kirin_ir::{ResultValue, Statement};

use crate::cursor::{ExecutionCursor, InternalSeed};

#[derive(Debug)]
pub(crate) struct Continuation {
    completed_statement: Statement,
    resume: InternalSeed,
    results: Vec<ResultValue>,
}

impl Continuation {
    pub(crate) fn new(
        completed_statement: Statement,
        resume: InternalSeed,
        results: Vec<ResultValue>,
    ) -> Self {
        Self {
            completed_statement,
            resume,
            results,
        }
    }

    pub(crate) fn completed_statement(&self) -> Statement {
        self.completed_statement
    }

    pub(crate) fn resume(&self) -> InternalSeed {
        self.resume
    }

    pub(crate) fn results(&self) -> &[ResultValue] {
        &self.results
    }
}

#[derive(Debug)]
pub(crate) struct Activation {
    pub(crate) cursor_stack: Vec<ExecutionCursor>,
    pub(crate) after_statement: Option<Statement>,
    pub(crate) continuation: Option<Continuation>,
}

impl Activation {
    pub(crate) fn new(cursor: ExecutionCursor, continuation: Option<Continuation>) -> Self {
        Self {
            cursor_stack: vec![cursor],
            after_statement: None,
            continuation,
        }
    }
}
