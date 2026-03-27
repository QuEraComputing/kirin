use indexmap::IndexMap;

use crate::arena::Id;
use crate::identifier;

use super::compile_stage::CompileStage;
use super::staged::StagedFunction;
use crate::node::symbol::GlobalSymbol;

identifier! {
    /// A unique identifier for a generic function.
    ///
    /// Functions can have multiple staged versions corresponding to different
    /// compilation stages.
    struct Function
}

/// Information about a function across different compilation stages.
#[derive(Clone, Debug)]
pub struct FunctionInfo {
    id: Function,
    name: Option<GlobalSymbol>,
    /// compiled versions of the function at different stages.
    ///
    /// note that compile stages may not be sequential,
    /// i.e., some stages may be skipped when a user directly programs a low-level stage
    /// language and modifies the compilation stage accordingly.
    ///
    /// Some early stages may be discarded later in the compilation pipeline to save memory.
    ///
    /// The execution will always look for the matching stage of the target execution environment
    /// e.g an interpreter will look for the staged function at the interpreter stage.
    /// but LLVM backend will look for the staged function at the LLVM IR generation stage.
    staged_functions: IndexMap<CompileStage, StagedFunction>,
}

impl FunctionInfo {
    pub fn new(id: Function, name: Option<GlobalSymbol>) -> Self {
        Self {
            id,
            name,
            staged_functions: IndexMap::new(),
        }
    }

    pub fn id(&self) -> Function {
        self.id
    }

    pub fn name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    pub fn staged_functions(&self) -> &IndexMap<CompileStage, StagedFunction> {
        &self.staged_functions
    }

    pub fn staged_function(&self, stage: CompileStage) -> Option<StagedFunction> {
        self.staged_functions.get(&stage).copied()
    }

    pub fn add_staged_function(&mut self, stage: CompileStage, func: StagedFunction) {
        self.staged_functions.insert(stage, func);
    }
}

impl From<FunctionInfo> for Function {
    fn from(fi: FunctionInfo) -> Self {
        fi.id
    }
}
