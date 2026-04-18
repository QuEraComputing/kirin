use std::convert::Infallible;

use kirin_ir::{Block, CompileStage, ResultValue, SpecializedFunction};

/// Structural execution effect, dialect-independent.
///
/// `V` is the value type.
/// `Ext` is the interpreter-mode extension type:
///   - Concrete: `ControlExt<C>` (cursor push/pop events)
///   - Abstract:  `Infallible` (proves no cursor events occur at the type level)
///
/// `Control::Advance` and `Control::Jump` are consumed inside
/// `BlockCursor::execute` and do not reach the driver loop.
/// All other variants are handled by the driver loop.
pub enum Control<V, Ext = Infallible> {
    /// Advance to the next statement (handled inside BlockCursor, not by driver).
    Advance,
    /// Jump to another block with arguments (handled inside BlockCursor).
    Jump(Block, Vec<V>),
    /// Both successors are possible (abstract nondeterministic branch).
    ///
    /// The concrete driver returns an error if this variant is received.
    /// The abstract driver adds both targets to the worklist.
    Fork(Block, Vec<V>, Block, Vec<V>),
    /// Call a specialized function.
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    },
    /// Return from the current function.
    Return(V),
    /// Yield a value from an inline body (e.g. scf.if / scf.for body).
    Yield(V),
    /// Interpreter-mode extension event (cursor push/pop for concrete; never
    /// occurs for abstract since `Ext = Infallible`).
    Ext(Ext),
}

impl<V, Ext> Control<V, Ext> {
    /// Map the extension type via a function.
    ///
    /// Used by dialect wrappers to convert `Control<V, Infallible>` (from pure
    /// CF ops) into `Control<V, E::Ext>` (the wrapper's return type). Since
    /// flat CF ops never produce `Control::Ext(...)`, the mapping function is
    /// never actually called — the compiler optimises away the dead branch.
    pub fn map_ext<Ext2>(self, f: impl FnOnce(Ext) -> Ext2) -> Control<V, Ext2> {
        match self {
            Control::Advance => Control::Advance,
            Control::Jump(b, a) => Control::Jump(b, a),
            Control::Fork(b1, a1, b2, a2) => Control::Fork(b1, a1, b2, a2),
            Control::Call {
                callee,
                stage,
                args,
                results,
            } => Control::Call {
                callee,
                stage,
                args,
                results,
            },
            Control::Return(v) => Control::Return(v),
            Control::Yield(v) => Control::Yield(v),
            Control::Ext(e) => Control::Ext(f(e)),
        }
    }
}

/// Cursor stack events for concrete execution.
///
/// Produced by SCF dialect ops (e.g. `If`, `For`) when they push a body
/// cursor onto the cursor stack. `Pop` is produced by the body cursor itself
/// when it completes.
pub enum ControlExt<C> {
    /// Push a new cursor onto the cursor stack.
    Push(C),
    /// Remove the current cursor from the stack without side effects.
    Pop,
}

/// Unit `()` converts to `Control::Advance`.
///
/// Dialect ops that only perform side-effects (writing SSA results) and
/// always advance return `()` from `interpret`. Dialect wrappers convert via
/// `op.interpret(env).map(Control::from)`.
impl<V, Ext> From<()> for Control<V, Ext> {
    fn from(_: ()) -> Self {
        Control::Advance
    }
}

/// `ControlExt<C>` from `Infallible` — needed so that
/// `Control<V, Infallible>.map_ext(ControlExt::from)` compiles for wrappers
/// that accept both abstract and concrete modes.
///
/// The `Infallible` variant of `ControlExt` is unreachable at runtime — the
/// compiler elides the dead branch.
impl<C> From<Infallible> for ControlExt<C> {
    fn from(e: Infallible) -> Self {
        match e {}
    }
}
