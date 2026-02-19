use kirin_ir::{Block, ResultValue, SpecializedFunction};

/// Shared control constructors available to all interpreter implementations.
///
/// Dialect `Interpretable` impls that only need these common actions can be
/// generic over `I: Interpreter` without knowing the concrete control type.
pub trait InterpretControl<V>: Sized {
    /// Advance to the next statement in the current block.
    fn ctrl_continue() -> Self;
    /// Jump to a target block, binding argument values to its block arguments.
    fn ctrl_jump(block: Block, args: Vec<V>) -> Self;
    /// Return a single value from the current function frame.
    fn ctrl_return(value: V) -> Self;
    /// Call a specialized function with arguments, writing the return value
    /// to `result` in the caller's frame.
    fn ctrl_call(callee: SpecializedFunction, args: Vec<V>, result: ResultValue) -> Self;

    /// Fork into multiple targets (undecidable branch).
    ///
    /// Default: panics. Only abstract interpreters provide a meaningful
    /// implementation. Concrete interpreters should never reach an
    /// undecidable branch because [`BranchCondition::is_truthy`] always
    /// returns `Some` for concrete values.
    fn ctrl_fork(targets: Vec<(Block, Vec<V>)>) -> Self {
        let _ = targets;
        panic!("ctrl_fork is only supported by abstract interpreters")
    }
}

/// Control protocol for concrete (stack-based) interpretation.
///
/// Returned by dialect `Interpretable` impls when the interpreter is a
/// [`crate::StackInterpreter`]. Does **not** include `Fork` — undecidable
/// branches are a compile-time error in concrete mode.
#[derive(Debug)]
pub enum ConcreteControl<V> {
    /// Advance to the next statement in the current block.
    Continue,
    /// Jump to a target block, binding argument values to its block arguments.
    Jump(Block, Vec<V>),
    /// Call a concrete specialized function with arguments.
    Call {
        callee: SpecializedFunction,
        args: Vec<V>,
        /// Where to write the return value in the caller's frame.
        result: ResultValue,
    },
    /// Return a single value from the current function frame.
    Return(V),
    /// Suspend execution at the current statement (debugger breakpoint).
    Break,
    /// Terminate the session.
    Halt,
}

impl<V> InterpretControl<V> for ConcreteControl<V> {
    fn ctrl_continue() -> Self {
        ConcreteControl::Continue
    }
    fn ctrl_jump(block: Block, args: Vec<V>) -> Self {
        ConcreteControl::Jump(block, args)
    }
    fn ctrl_return(value: V) -> Self {
        ConcreteControl::Return(value)
    }
    fn ctrl_call(callee: SpecializedFunction, args: Vec<V>, result: ResultValue) -> Self {
        ConcreteControl::Call {
            callee,
            args,
            result,
        }
    }
}

/// Control protocol for abstract (fixpoint) interpretation.
///
/// Returned by dialect `Interpretable` impls when the interpreter is an
/// [`crate::AbstractInterpreter`]. Includes `Fork` for undecidable branches
/// but does **not** include `Break` or `Halt`.
#[derive(Debug)]
pub enum AbstractControl<V> {
    /// Advance to the next statement in the current block.
    Continue,
    /// Jump to a target block, binding argument values to its block arguments.
    Jump(Block, Vec<V>),
    /// Call a specialized function with arguments.
    Call {
        callee: SpecializedFunction,
        args: Vec<V>,
        /// Where to write the return value in the caller's frame.
        result: ResultValue,
    },
    /// Fork into multiple targets (undecidable branch).
    Fork(Vec<(Block, Vec<V>)>),
    /// Return a single value from the current function frame.
    Return(V),
}

impl<V> InterpretControl<V> for AbstractControl<V> {
    fn ctrl_continue() -> Self {
        AbstractControl::Continue
    }
    fn ctrl_jump(block: Block, args: Vec<V>) -> Self {
        AbstractControl::Jump(block, args)
    }
    fn ctrl_return(value: V) -> Self {
        AbstractControl::Return(value)
    }
    fn ctrl_call(callee: SpecializedFunction, args: Vec<V>, result: ResultValue) -> Self {
        AbstractControl::Call {
            callee,
            args,
            result,
        }
    }
    fn ctrl_fork(targets: Vec<(Block, Vec<V>)>) -> Self {
        AbstractControl::Fork(targets)
    }
}
