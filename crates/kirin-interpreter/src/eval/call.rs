use kirin_ir::{
    Block, Dialect, HasStageInfo, SpecializedFunction, StageInfo, StageMeta, SupportsStageDispatch,
};

use crate::{Interpreter, InterpreterError};

/// Trait for customizing how a function body is executed.
///
/// Different body types can provide different execution strategies:
/// - SSA CFG bodies get a blanket impl via [`SSACFGRegion`]
/// - Non-SSA bodies (e.g. circuit graphs) can implement this directly
///
/// `L` is the composed dialect enum that this body is part of.
pub trait EvalCall<'ir, I: Interpreter<'ir>, L: Dialect>: Dialect {
    type Result;

    fn eval_call(
        &self,
        interpreter: &mut I,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, I::Error>;
}

/// Marker trait for body types that represent SSA CFG regions.
///
/// Implementing this trait provides blanket [`EvalCall`] impls for both
/// [`crate::StackInterpreter`] and [`crate::AbstractInterpreter`], using the
/// standard CFG traversal / fixpoint computation logic.
pub trait SSACFGRegion: Dialect {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError>;
}

// ---------------------------------------------------------------------------
// Blanket impl: SSACFGRegion → EvalCall<StackInterpreter>
// ---------------------------------------------------------------------------

impl<'ir, V, S, E, G, L, T> EvalCall<'ir, crate::StackInterpreter<'ir, V, S, E, G>, L> for T
where
    T: SSACFGRegion,
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
    L: Dialect + crate::Interpretable<'ir, crate::StackInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    type Result = V;

    fn eval_call(
        &self,
        interp: &mut crate::StackInterpreter<'ir, V, S, E, G>,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<V, E> {
        let entry = self.entry_block::<L>(stage)?;

        // Push frame and bind entry block args
        let first = entry.first_statement(stage);
        let frame_stage = stage.stage_id().unwrap_or_else(|| interp.active_stage());
        interp.push_frame(crate::Frame::new(callee, frame_stage, first))?;
        crate::EvalBlock::bind_block_args(interp, stage, entry, args)?;

        let initial_depth = interp.frame_depth();
        let mut pending_results: Vec<kirin_ir::ResultValue> = Vec::new();

        loop {
            let control = interp.run()?;
            match &control {
                crate::Continuation::Call { result, .. } => pending_results.push(*result),
                crate::Continuation::Ext(crate::ConcreteExt::Halt) => {
                    return Err(
                        InterpreterError::UnexpectedControl("halt during call".to_owned()).into(),
                    );
                }
                crate::Continuation::Return(_) | crate::Continuation::Yield(_) => {}
                _ => {
                    return Err(InterpreterError::UnexpectedControl(
                        "unexpected variant during call".to_owned(),
                    )
                    .into());
                }
            }

            let v = match &control {
                crate::Continuation::Return(v) | crate::Continuation::Yield(v) => Some(v.clone()),
                _ => None,
            };

            interp.advance(&control)?;

            if let Some(v) = v {
                if interp.frame_depth() < initial_depth {
                    return Ok(v);
                }
                let result = pending_results
                    .pop()
                    .ok_or_else(|| InterpreterError::NoFrame.into())?;
                <crate::StackInterpreter<'ir, V, S, E, G> as Interpreter<'ir>>::write(
                    interp, result, v,
                )?;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Blanket impl: SSACFGRegion → EvalCall<AbstractInterpreter>
// ---------------------------------------------------------------------------

impl<'ir, V, S, E, G, L, T> EvalCall<'ir, crate::AbstractInterpreter<'ir, V, S, E, G>, L> for T
where
    T: SSACFGRegion,
    V: crate::AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect + crate::Interpretable<'ir, crate::AbstractInterpreter<'ir, V, S, E, G>, L> + 'ir,
{
    type Result = crate::AnalysisResult<V>;

    fn eval_call(
        &self,
        interp: &mut crate::AbstractInterpreter<'ir, V, S, E, G>,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<crate::AnalysisResult<V>, E> {
        let entry = self.entry_block::<L>(stage)?;
        let stage_id = stage.stage_id().unwrap_or_else(|| interp.active_stage());

        // Insert tentative summary before pushing frame
        interp.set_tentative(stage_id, callee, args, crate::AnalysisResult::bottom());

        // Outer fixpoint loop for inter-procedural convergence
        let max_iters = interp.max_summary_iterations();
        let mut summary_iterations = 0;
        let final_result = loop {
            summary_iterations += 1;
            if summary_iterations > max_iters {
                return Err(InterpreterError::FuelExhausted.into());
            }

            // Push frame and run forward analysis
            interp.push_frame(crate::Frame::new(
                callee,
                stage_id,
                crate::FixpointState::default(),
            ))?;
            let result = interp.run_forward::<L>(stage_id, entry, args);
            let _ = interp.pop_frame()?;

            let result = result?;

            // Check convergence against old tentative result
            let old_result = interp.tentative_result(stage_id, callee).cloned();

            // Update tentative summary
            interp.set_tentative(stage_id, callee, args, result.clone());

            // Converged if all block argument states and return value stabilized
            let converged = match old_result {
                Some(ref old) => result.is_subseteq(old),
                None => summary_iterations > 1,
            };

            if converged {
                break result;
            }
        };

        // Promote tentative to computed entry
        interp.promote_tentative(stage_id, callee, args, final_result.clone());

        Ok(final_result)
    }
}
