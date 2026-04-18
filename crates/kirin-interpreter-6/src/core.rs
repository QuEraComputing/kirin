use kirin_ir::{Block, CompileStage, ResultValue, SpecializedFunction};

use crate::lift::Lift;

/// Structural execution effects, dialect-independent.
///
/// `V` is the value type. `C` is the cursor type pushed onto the cursor stack —
/// in a composed language this is the language's cursor coproduct (e.g.
/// `MyLangCursor<V>`). `C` is `V`-parameterized only, not interpreter-parameterized,
/// so `Core` carries no reference to the interpreter type.
///
/// `Core::Advance` and `Core::Jump` are handled *inside* `BlockCursor::execute`
/// and should not normally reach the driver loop. All other variants are handled
/// by the driver loop.
pub enum Core<V, C = ()> {
    /// Advance to the next statement (handled inside BlockCursor, not by driver loop).
    Advance,
    /// Jump to another block with arguments (handled inside BlockCursor).
    Jump(Block, Vec<V>),
    /// Return from the current function.
    Return(V),
    /// Yield a value from the current inline execution (e.g. scf.for body).
    Yield(V),
    /// Push a new cursor entry onto the cursor stack.
    Push(C),
    /// Remove the current cursor from the stack without side effects.
    Pop,
    /// Call a specialized function, writing results to caller result slots.
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    },
}

/// Unit `()` lifts to `Core::Advance` — convenience for ops that simply advance.
///
/// Dialect ops whose only effect is "move to the next statement" can return `()`
/// and declare `type DialectEffect = ()`, requiring `E::Effect: Lift<()>`.
impl<V, C> Lift<()> for Core<V, C> {
    fn lift(_: ()) -> Self {
        Core::Advance
    }
}
