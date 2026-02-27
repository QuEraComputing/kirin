use kirin_ir::{
    Block, Dialect, GetInfo, HasStageInfo, ResultValue, SSAValue, StageInfo, StageMeta,
};

use crate::{
    AbstractInterpreter, AbstractValue, Continuation, Interpretable, Interpreter, InterpreterError,
    StackInterpreter,
};

/// Trait for interpreters that can execute a body block inline and return the
/// resulting [`Continuation`].
///
/// Callers are responsible for binding block arguments via
/// [`bind_block_args`](Self::bind_block_args) before calling
/// [`eval_block`](Self::eval_block).
pub trait EvalBlock<'ir, L: Dialect>: Interpreter<'ir> {
    /// Execute a body block whose arguments have already been bound.
    ///
    /// Returns the [`Continuation`] produced by the block's terminator.
    /// The caller must call [`bind_block_args`](Self::bind_block_args) first
    /// to write values into the block's argument SSA slots.
    fn eval_block(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>;

    /// Bind values to a block's arguments in the current frame.
    ///
    /// Resolves the block's argument SSA values from stage info and writes
    /// each provided value. Returns `ArityMismatch` if `args.len()` differs
    /// from the block's declared argument count.
    fn bind_block_args(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
        args: &[Self::Value],
    ) -> Result<(), Self::Error>
    where
        Self::Value: Clone,
        Self::Error: From<InterpreterError>,
    {
        let block_info = block.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }
        let arg_ssas: Vec<SSAValue> = block_info
            .arguments
            .iter()
            .map(|ba| SSAValue::from(*ba))
            .collect();
        for (ssa, val) in arg_ssas.iter().zip(args.iter()) {
            self.write_ssa(*ssa, val.clone())?;
        }
        Ok(())
    }
}

impl<'ir, V, S, E, G, L> EvalBlock<'ir, L> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + Interpretable<'ir, Self, L>,
{
    /// Execute a body block inline within the current frame, handling nested
    /// calls. Returns `Continuation::Yield(v)` with the value produced by the
    /// block's yield terminator.
    ///
    /// This mirrors the call-semantics loop in [`EvalCall`] but operates
    /// within the current frame rather than pushing a new one. Used by
    /// structured control flow operations like `scf.for` that need to
    /// repeatedly execute a body block.
    fn eval_block(
        &mut self,
        stage: &'ir StageInfo<L>,
        block: Block,
    ) -> Result<Continuation<V, crate::ConcreteExt>, E> {
        // Save current cursor so we can restore it after the block completes
        let saved_cursor = self.current_frame()?.cursor();

        // Set cursor to the block's first statement
        let first = block.first_statement(stage);
        self.current_frame_mut()?.set_cursor(first);

        // Run body, handling nested Call/Return pairs (same pattern as EvalCall)
        let mut pending_results: Vec<ResultValue> = Vec::new();
        loop {
            let control = self.run::<L>()?;
            match &control {
                Continuation::Call { result, .. } => {
                    pending_results.push(*result);
                    self.advance::<L>(&control)?;
                }
                Continuation::Yield(v) => {
                    let v = v.clone();
                    self.current_frame_mut()?.set_cursor(saved_cursor);
                    return Ok(Continuation::Yield(v));
                }
                Continuation::Return(v) => {
                    // Return from a nested call — pop the callee frame
                    let v = v.clone();
                    self.advance::<L>(&control)?;
                    let result = pending_results.pop().ok_or(InterpreterError::NoFrame)?;
                    Interpreter::write(self, result, v)?;
                }
                _ => {
                    return Err(InterpreterError::UnexpectedControl(
                        "unexpected continuation in SCF body block".to_owned(),
                    )
                    .into());
                }
            }
        }
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
                    args,
                    result,
                } => {
                    let handler = self
                        .call_handler
                        .expect("call_handler not set: analyze() must be used as entry point");
                    let analysis = handler(self, callee, &args)?;
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
