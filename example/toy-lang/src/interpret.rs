use kirin::prelude::*;
use kirin_interpreter::{InterpreterError, SSACFGRegion};

use crate::language::{HighLevel, LowLevel};

// ---------------------------------------------------------------------------
// HighLevel: SSACFGRegion (provides blanket CallSemantics)
// ---------------------------------------------------------------------------

impl SSACFGRegion for HighLevel {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError> {
        match self {
            HighLevel::Lexical(inner) => inner.entry_block(stage),
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: SSACFGRegion (provides blanket CallSemantics)
// ---------------------------------------------------------------------------

impl SSACFGRegion for LowLevel {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError> {
        match self {
            LowLevel::Lifted(inner) => inner.entry_block(stage),
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}
