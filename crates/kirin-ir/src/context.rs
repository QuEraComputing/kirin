use crate::{FunctionInfo, InternTable, Symbol};

/// Context holding information about functions, blocks, statements, and SSA values.
///
/// Stage: the type representing different compilation stages, should be defined as an
/// enum over different `IRContext`s over languages.
#[derive(Clone, Debug)]
pub struct Context<StageInfo> {
    stages: Vec<StageInfo>,
    functions: Vec<FunctionInfo>,
    interned_symbols: InternTable<String, Symbol>,
}

impl<StageInfo> Default for Context<StageInfo> {
    fn default() -> Self {
        Self {
            stages: Vec::new(),
            functions: Vec::new(),
            interned_symbols: InternTable::default(),
        }
    }
}

impl<StageInfo> Context<StageInfo> {
    /// Get the stages in the context.
    pub fn stages(&self) -> &Vec<StageInfo> {
        &self.stages
    }

    /// Get the functions in the context.
    pub fn functions(&self) -> &Vec<FunctionInfo> {
        &self.functions
    }

    /// Get the interned symbols table.
    pub fn interned_symbols(&self) -> &InternTable<String, Symbol> {
        &self.interned_symbols
    }
}
