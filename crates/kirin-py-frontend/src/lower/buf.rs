use kirin::prelude::Statement;

/// Accumulates the statements (in evaluation order) and the optional terminator
/// for a block under construction. The block is created args-first, statements
/// are collected here, then attached via `attach_statements_to_block`.
pub(crate) struct BlockBuf {
    pub stmts: Vec<Statement>,
    pub terminator: Option<Statement>,
}

impl BlockBuf {
    pub fn new() -> Self {
        Self {
            stmts: Vec::new(),
            terminator: None,
        }
    }

    /// Append a non-terminator statement.
    pub fn push(&mut self, stmt: Statement) {
        self.stmts.push(stmt);
    }

    /// Record the block terminator (e.g. `ret` / `yield`).
    pub fn set_terminator(&mut self, stmt: Statement) {
        self.terminator = Some(stmt);
    }
}
