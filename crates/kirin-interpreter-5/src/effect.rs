use kirin_ir::{Block, CompileStage, ResultValue, SpecializedFunction};

/// The unified effect type for interpreter-5.
///
/// `V` is the value type. `C` is the cursor type pushed onto the global cursor
/// stack (defaults to `()` for single-stage use).
pub enum ControlFlow<V, C = ()> {
    /// Advance to the next statement in the current block.
    Advance,
    /// Jump the cursor to a different block with the given arguments.
    Jump(Block, Vec<V>),
    /// Return from the current function, writing the value to caller results.
    Return(V),
    /// Yield a value from the current inline execution.
    Yield(V),
    /// Push a new cursor entry onto the global cursor stack.
    Push(C),
    /// Remove the current cursor from the stack without side effects.
    Pop,
    /// Call a specialized function with arguments, writing results to the given slots.
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    },
}
