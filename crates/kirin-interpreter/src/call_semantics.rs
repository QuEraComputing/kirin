use kirin_ir::{
    Block, CompileStageInfo, Dialect, GetInfo, HasStageInfo, SpecializedFunction, StageInfo,
};

use crate::{Interpreter, InterpreterError};

/// Trait for customizing how a function body is executed.
///
/// Different body types can provide different execution strategies:
/// - SSA CFG bodies get a blanket impl via [`SSACFGRegion`]
/// - Non-SSA bodies (e.g. circuit graphs) can implement this directly
///
/// `L` is the composed dialect enum that this body is part of.
pub trait CallSemantics<I: Interpreter, L: Dialect>: Dialect {
    type Result;

    fn call_semantics(
        &self,
        interpreter: &mut I,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, I::Error>;
}

/// Marker trait for body types that represent SSA CFG regions.
///
/// Implementing this trait provides blanket [`CallSemantics`] impls for both
/// [`crate::StackInterpreter`] and [`crate::AbstractInterpreter`], using the
/// standard CFG traversal / fixpoint computation logic.
pub trait SSACFGRegion: Dialect {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError>;
}

// ---------------------------------------------------------------------------
// Blanket impl: SSACFGRegion → CallSemantics<StackInterpreter>
// ---------------------------------------------------------------------------

impl<'ir, V, S, E, G, L, T> CallSemantics<crate::StackInterpreter<'ir, V, S, E, G>, L> for T
where
    T: SSACFGRegion,
    V: Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo + HasStageInfo<L>,
    G: 'ir,
    L: Dialect + crate::Interpretable<crate::StackInterpreter<'ir, V, S, E, G>, L>,
{
    type Result = V;

    fn call_semantics(
        &self,
        interp: &mut crate::StackInterpreter<'ir, V, S, E, G>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<V, E> {
        let stage = interp.resolve_stage::<L>();
        let entry = self.entry_block::<L>(stage)?;

        // Push frame with entry block cursor and bind args
        let first = interp.first_stmt_in_block::<L>(entry);
        let mut frame = crate::Frame::new(callee, first);
        if let Some(entry_stmt) = first {
            let stage = interp.resolve_stage::<L>();
            let parent_block = *entry_stmt.parent::<L>(stage);
            if let Some(block) = parent_block {
                let block_info = block.expect_info(stage);
                if block_info.arguments.len() != args.len() {
                    return Err(InterpreterError::ArityMismatch {
                        expected: block_info.arguments.len(),
                        got: args.len(),
                    }
                    .into());
                }
                for (ba, val) in block_info.arguments.iter().zip(args) {
                    frame.write_ssa(kirin_ir::SSAValue::from(*ba), V::clone(val));
                }
            }
        }
        interp.push_call_frame(frame)?;

        let initial_depth = interp.frames_len();
        let mut pending_results: Vec<kirin_ir::ResultValue> = Vec::new();

        loop {
            let control = interp.run::<L>()?;
            match &control {
                crate::Continuation::Call { result, .. } => pending_results.push(*result),
                crate::Continuation::Ext(crate::ConcreteExt::Halt) => {
                    return Err(
                        InterpreterError::UnexpectedControl("halt during call".to_owned()).into(),
                    );
                }
                crate::Continuation::Return(_) => {}
                _ => {
                    return Err(InterpreterError::UnexpectedControl(
                        "unexpected variant during call".to_owned(),
                    )
                    .into());
                }
            }

            let v = match &control {
                crate::Continuation::Return(v) => Some(v.clone()),
                _ => None,
            };

            interp.advance::<L>(&control)?;

            if let Some(v) = v {
                if interp.frames_len() < initial_depth {
                    return Ok(v);
                }
                let result = pending_results
                    .pop()
                    .ok_or_else(|| InterpreterError::NoFrame.into())?;
                <crate::StackInterpreter<'ir, V, S, E, G> as Interpreter>::write(
                    interp, result, v,
                )?;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Blanket impl: SSACFGRegion → CallSemantics<AbstractInterpreter>
// ---------------------------------------------------------------------------

impl<'ir, V, S, E, G, L, T> CallSemantics<crate::AbstractInterpreter<'ir, V, S, E, G>, L> for T
where
    T: SSACFGRegion,
    V: crate::AbstractValue + Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo + HasStageInfo<L>,
    G: 'ir,
    L: Dialect + crate::Interpretable<crate::AbstractInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    type Result = crate::AnalysisResult<V>;

    fn call_semantics(
        &self,
        interp: &mut crate::AbstractInterpreter<'ir, V, S, E, G>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<crate::AnalysisResult<V>, E> {
        let stage = interp.resolve_stage::<L>();
        let entry = self.entry_block::<L>(stage)?;

        // Insert tentative summary before pushing frame
        interp.set_tentative(callee, args, crate::AnalysisResult::bottom());

        // Outer fixpoint loop for inter-procedural convergence
        let max_iters = interp.max_summary_iterations();
        let mut summary_iterations = 0;
        let final_result = loop {
            summary_iterations += 1;
            if summary_iterations > max_iters {
                return Err(InterpreterError::FuelExhausted.into());
            }

            // Push frame and run forward analysis
            interp.push_analysis_frame(callee);
            let result = interp.run_forward::<L>(entry, args);
            interp.pop_analysis_frame();

            let result = result?;

            // Check if summary changed
            let old_return = interp.tentative_return_value(callee).cloned();
            let new_return = result.return_value().cloned();

            // Update tentative summary
            interp.set_tentative(callee, args, result.clone());

            // Converged if return value stabilized
            let converged = match (&old_return, &new_return) {
                (Some(old), Some(new)) => new.is_subseteq(old),
                (None, None) => true,
                _ => summary_iterations > 1,
            };

            if converged {
                break result;
            }
        };

        // Promote tentative to computed entry
        interp.promote_tentative(callee, args, final_result.clone());

        Ok(final_result)
    }
}
