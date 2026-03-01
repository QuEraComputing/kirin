use kirin_ir::{Block, Dialect, HasStageInfo, StageInfo, StageMeta, SupportsStageDispatch};

use crate::{
    AbstractInterpreter, AbstractValue, Continuation, Interpretable, Interpreter, InterpreterError,
    StackInterpreter,
};

/// Trait for interpreters that can execute a body block inline and return the
/// resulting [`Continuation`].
///
/// Callers are responsible for binding block arguments via
/// [`Interpreter::bind_block_args`] before calling
/// [`eval_block`](Self::eval_block).
pub trait EvalBlock<'ir, L: Dialect>: Interpreter<'ir> {
    /// Execute a body block whose arguments have already been bound.
    ///
    /// Returns the [`Continuation`] produced by the block's terminator.
    /// The caller must call [`Interpreter::bind_block_args`] first
    /// to write values into the block's argument SSA slots.
    fn eval_block(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>;
}

impl<'ir, V, S, E, G, L> EvalBlock<'ir, L> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    S: SupportsStageDispatch<
            crate::stack::FrameDispatchAction<'ir, V, S, E, G>,
            crate::stack::DynFrameDispatch<'ir, V, S, E, G>,
            E,
        >,
    for<'a> S:
        SupportsStageDispatch<crate::stack::PushCallFrameDynAction<'a, 'ir, V, S, E, G>, (), E>,
    G: 'ir,
    L: Dialect + Interpretable<'ir, Self, L>,
{
    /// Execute a body block inline within the current frame, handling nested
    /// calls. Returns `Continuation::Yield(v)` with the value produced by the
    /// block's yield terminator.
    fn eval_block(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<V, crate::ConcreteExt>, E> {
        let saved_cursor = self.current_cursor()?;
        let first = block.first_statement(stage);
        self.set_current_cursor(first)?;
        let v = self.run_nested_calls(|_interp, is_yield| is_yield)?;
        self.set_current_cursor(saved_cursor)?;
        Ok(Continuation::Yield(v))
    }
}

impl<'ir, V, S, E, G, L> EvalBlock<'ir, L> for AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, Self, L> + 'ir,
{
    /// Interpret all statements in a block sequentially, returning the
    /// final [`Continuation`] from the terminator.
    ///
    /// `Call` continuations from non-terminator statements are dispatched
    /// inline via the installed `call_handler`.
    fn eval_block(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<V, std::convert::Infallible>, E> {
        // Iterate statements directly using the stage reference
        for stmt in block.statements(stage) {
            let def: &L = stmt.definition(stage);
            let control = def.interpret(self)?;
            match control {
                Continuation::Continue => {}
                Continuation::Call {
                    callee,
                    stage: callee_stage,
                    args,
                    result,
                } => {
                    let handler = self
                        .call_handler
                        .expect("call_handler not set: analyze() must be used as entry point");
                    let analysis = handler(self, callee, callee_stage, &args)?;
                    let return_val = analysis.return_value().cloned().unwrap_or_else(V::bottom);
                    self.write(result, return_val)?;
                }
                other => return Ok(other),
            }
        }

        // Interpret the terminator
        if let Some(term) = block.terminator(stage) {
            let def: &L = term.definition(stage);
            let control = def.interpret(self)?;
            Ok(control)
        } else {
            Err(InterpreterError::MissingEntry.into())
        }
    }
}
