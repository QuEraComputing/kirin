use crate::arena::Id;
use crate::identifier;

identifier! {
    /// A unique identifier for a compilation stage.
    ///
    /// Compilation stages represent different phases in the compilation pipeline,
    /// such as parsing, optimization, code generation, etc.
    /// Can be used as a compile-time value in statement definitions.
    struct CompileStage
}

impl CompileStage {
    pub fn new(stage: Id) -> Self {
        CompileStage(stage)
    }
}
