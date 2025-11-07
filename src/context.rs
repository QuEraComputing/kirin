use crate::ir::*;
use crate::language::Language;

/// Context holding information about functions, blocks, statements, and SSA values.
///
/// Stage: the type representing different compilation stages, should be defined as an
/// enum over different `IRContext`s over languages.
#[derive(Clone, Debug)]
pub struct Context<Stage> {
    functions: Vec<FunctionInfo<Stage>>,
    languages: Vec<Stage>,
    interned_symbols: InternTable,
}

impl<Stage> Default for Context<Stage> {
    fn default() -> Self {
        Self {
            functions: Vec::new(),
            languages: Vec::new(),
            interned_symbols: InternTable::default(),
        }
    }
}

pub struct IRContext<L: Language> {
    staged_functions: Vec<StagedFunctionInfo<L>>,
    blocks: Vec<BlockInfo>,
    statements: Vec<StatementInfo<L>>,
    ssas: Vec<SSAInfo<L>>,
}

impl<L> Default for IRContext<L>
where
    L: Language,
{
    fn default() -> Self {
        Self {
            staged_functions: Vec::new(),
            blocks: Vec::new(),
            statements: Vec::new(),
            ssas: Vec::new(),
        }
    }
}

impl<L> Clone for IRContext<L>
where
    L: Language,
    StatementInfo<L>: Clone,
    SSAInfo<L>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            staged_functions: self.staged_functions.clone(),
            blocks: self.blocks.clone(),
            statements: self.statements.clone(),
            ssas: self.ssas.clone(),
        }
    }
}
