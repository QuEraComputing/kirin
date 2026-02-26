use kirin_ir::{Block, CompileStageInfo, Dialect, HasStageInfo, ResultValue};

use crate::{
    Continuation, Interpretable, Interpreter, InterpreterError, StackInterpreter,
};

/// Trait for interpreters that can execute a body block inline and return the
/// yielded value. Used by structured control flow operations like `scf.for`.
pub trait BlockExecutor<L: Dialect>: Interpreter {
    fn execute_block(
        &mut self,
        block: Block,
        args: &[Self::Value],
    ) -> Result<Self::Value, Self::Error>;
}

impl<'ir, V, S, E, G, L> BlockExecutor<L> for StackInterpreter<'ir, V, S, E, G>
where
    V: Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo + HasStageInfo<L>,
    L: Dialect + Interpretable<Self, L>,
{
    /// Execute a body block inline within the current frame, handling nested
    /// calls. Returns the value produced by the block's terminator (e.g.
    /// `Yield` returning `Continuation::Return(v)`).
    ///
    /// This mirrors the call-semantics loop in [`CallSemantics`] but operates
    /// within the current frame rather than pushing a new one. Used by
    /// structured control flow operations like `scf.for` that need to
    /// repeatedly execute a body block.
    fn execute_block(
        &mut self,
        block: Block,
        args: &[V],
    ) -> Result<V, E> {
        // Save current cursor so we can restore it after the block completes
        let saved_cursor = self.current_frame()?.cursor();

        // Jump to block with args
        self.bind_block_args::<L>(block, args)?;
        let first = self.first_stmt_in_block::<L>(block);
        self.current_frame_mut()?.set_cursor(first);

        // Run body, handling nested Call/Return pairs (same pattern as CallSemantics)
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
                    return Ok(v);
                }
                Continuation::Return(v) => {
                    // Return from a nested call — pop the callee frame
                    let v = v.clone();
                    self.advance::<L>(&control)?;
                    let result = pending_results
                        .pop()
                        .ok_or(InterpreterError::NoFrame)?;
                    Interpreter::write(self, result, v)?;
                }
                _ => {
                    return Err(InterpreterError::UnexpectedControl(
                        "unexpected continuation in SCF body block".to_owned(),
                    )
                    .into())
                }
            }
        }
    }
}
