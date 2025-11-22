use crate::{Arena, CompileStage, Function, FunctionInfo, InternTable, Language, Symbol};

pub trait StageInfo {}

pub struct SingletonStage<L: Language>(Arena<L>);

/// Context holding information about functions, blocks, statements, and SSA values.
///
/// Stage: the type representing different compilation stages, should be defined as an
/// enum over different `IRContext`s over languages.
#[derive(Clone, Debug)]
pub struct Context<S: StageInfo> {
    stages: Vec<S>,
    functions: Vec<FunctionInfo>,
    interned_symbols: InternTable<String, Symbol>,
}

impl<S: StageInfo> Default for Context<S> {
    fn default() -> Self {
        Self {
            stages: Vec::new(),
            functions: Vec::new(),
            interned_symbols: InternTable::default(),
        }
    }
}

impl<S: StageInfo> Context<S> {
    // pub fn add_stage(&mut self, mut stage: S) -> CompileStage {
    //     let id = self.stages.len();
    //     for f in &self.functions {
    //         stage.add_function(f);
    //     }
    //     self.stages.push(stage);
    //     CompileStage(id)
    // }

    pub fn add_function(&mut self, f: FunctionInfo) -> Function {
        let id = self.functions.len();
        self.functions.push(f);
        for stage in &mut self.stages {
            stage.add_function(&self.functions[id]);
        }
        Function(id)
    }

    pub fn add_symbol(&mut self, name: impl AsRef<str>) -> Symbol {
        self.interned_symbols.intern(name.as_ref().to_string())
    }

    /// Get the stages in the context.
    pub fn stages(&self) -> &Vec<S> {
        &self.stages
    }

    pub fn get_stage(&self, stage: CompileStage) -> Option<&S> {
        self.stages.get(stage.id())
    }

    pub fn get_function(&self, f: impl Into<Function>) -> Option<&FunctionInfo> {
        self.functions.get(f.into().id())
    }

    pub fn get_function_mut(&mut self, f: impl Into<Function>) -> Option<&mut FunctionInfo> {
        self.functions.get_mut(f.into().id())
    }

    pub fn get_symbol_name(&self, symbol: Symbol) -> Option<&String> {
        self.interned_symbols.resolve(symbol)
    }
}
