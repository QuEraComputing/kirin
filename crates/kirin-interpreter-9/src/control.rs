use kirin_ir::{Block, CompileStage, ResultValue, SpecializedFunction};

/// Structural execution effect, dialect-independent.
///
/// `V` is the value type. `Ext` is the interpreter-mode extension type,
/// typically `CursorExt<C>` for both concrete and abstract interpreters.
pub enum Control<V, Ext> {
    /// Advance to the next statement (handled inside BlockCursor).
    Advance,
    /// Jump to another block with arguments (handled inside BlockCursor).
    Jump(Block, Vec<V>),
    /// N-way nondeterministic branch.
    ///
    /// Concrete driver errors on this; abstract driver propagates all branches.
    Fork(Vec<(Block, Vec<V>)>),
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
    /// Interpreter-mode extension event (cursor push/pop).
    Ext(Ext),
}

impl<V, Ext> Control<V, Ext> {
    pub fn map_ext<Ext2>(self, f: impl FnOnce(Ext) -> Ext2) -> Control<V, Ext2> {
        match self {
            Control::Advance => Control::Advance,
            Control::Jump(b, a) => Control::Jump(b, a),
            Control::Fork(branches) => Control::Fork(branches),
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

/// Cursor stack events.
///
/// Used by SCF dialect ops (e.g. `If`, `For`) when they push a body
/// cursor onto the cursor stack.
pub enum CursorExt<C> {
    /// Push a new cursor onto the cursor stack.
    Push(C),
    /// Remove the current cursor from the stack.
    Pop,
}

impl<V, Ext> From<()> for Control<V, Ext> {
    fn from(_: ()) -> Self {
        Control::Advance
    }
}
