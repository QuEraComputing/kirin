use kirin_ir::{Block, Statement};

use crate::{StageAccess, control::Location};

/// Read-only execution-position inspection for typed shells and typed stage views.
pub trait Position<'ir>: StageAccess<'ir> {
    fn cursor_depth(&self) -> usize;

    fn current_block(&self) -> Option<Block>;

    fn current_statement(&self) -> Option<Statement>;

    fn current_location(&self) -> Option<Location>;
}
