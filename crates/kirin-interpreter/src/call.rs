use kirin_ir::{Block, Dialect, HasStageInfo, SpecializedFunction, StageInfo, StageMeta};
use smallvec::SmallVec;

use crate::{BlockEvaluator, Interpreter, InterpreterError, StageAccess};

/// Trait for customizing how a function body is executed.
///
/// Different body types can provide different execution strategies:
/// - SSA CFG bodies get a blanket impl via [`SSACFGRegion`]
/// - Non-SSA bodies (e.g. circuit graphs) can implement this directly
///
/// `L` is moved to the method level to avoid recursive trait resolution cycles.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `CallSemantics` for this interpreter",
    note = "implement `SSACFGRegion` for standard function body evaluation, or implement `CallSemantics` directly for custom call semantics"
)]
pub trait CallSemantics<'ir, I: Interpreter<'ir>>: Dialect {
    type Result;

    fn eval_call<L>(
        &self,
        interpreter: &mut I,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: crate::Interpretable<'ir, I> + CallSemantics<'ir, I, Result = Self::Result> + 'ir;
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
// Blanket impl: SSACFGRegion -> CallSemantics<StackInterpreter>
// ---------------------------------------------------------------------------

impl<'ir, V, S, E, G, T> CallSemantics<'ir, crate::StackInterpreter<'ir, V, S, E, G>> for T
where
    T: SSACFGRegion,
    V: Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Result = SmallVec<[V; 1]>;

    fn eval_call<L>(
        &self,
        interp: &mut crate::StackInterpreter<'ir, V, S, E, G>,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<SmallVec<[V; 1]>, E>
    where
        S: HasStageInfo<L>,
        E: From<InterpreterError>,
        L: crate::Interpretable<'ir, crate::StackInterpreter<'ir, V, S, E, G>>
            + CallSemantics<'ir, crate::StackInterpreter<'ir, V, S, E, G>, Result = SmallVec<[V; 1]>>
            + 'ir,
    {
        let entry = self.entry_block::<L>(stage)?;
        let first = entry.first_statement(stage);
        let frame_stage = interp.resolve_stage_id(stage);
        interp.push_frame(crate::Frame::new(callee, frame_stage, first))?;
        interp.bind_block_args(stage, entry, args)?;
        let initial_depth = interp.frame_depth();
        interp.run_nested_calls(|interp, _is_yield| interp.frame_depth() < initial_depth)
    }
}

// ---------------------------------------------------------------------------
// Blanket impl: SSACFGRegion -> CallSemantics<AbstractInterpreter>
// ---------------------------------------------------------------------------

impl<'ir, V, S, E, G, T> CallSemantics<'ir, crate::AbstractInterpreter<'ir, V, S, E, G>> for T
where
    T: SSACFGRegion,
    V: crate::AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    type Result = crate::AnalysisResult<V>;

    fn eval_call<L>(
        &self,
        interp: &mut crate::AbstractInterpreter<'ir, V, S, E, G>,
        stage: &'ir StageInfo<L>,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<crate::AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        E: From<InterpreterError>,
        L: crate::Interpretable<'ir, crate::AbstractInterpreter<'ir, V, S, E, G>>
            + CallSemantics<
                'ir,
                crate::AbstractInterpreter<'ir, V, S, E, G>,
                Result = crate::AnalysisResult<V>,
            > + 'ir,
    {
        let entry = self.entry_block::<L>(stage)?;
        let stage_id = interp.resolve_stage_id(stage);

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
            interp.frames.push(crate::Frame::new(
                callee,
                stage_id,
                crate::FixpointState::default(),
            ))?;
            let result = interp.run_forward::<L>(stage_id, entry, args);
            // run_forward pops the frame on success; pop on error to maintain
            // the frame stack invariant.
            if result.is_err() {
                let _ = interp.frames.pop::<E>();
            }
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
