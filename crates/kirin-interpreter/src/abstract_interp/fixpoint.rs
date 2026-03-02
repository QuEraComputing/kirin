use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, SSAValue, SpecializedFunction,
    StageAction, StageInfo, StageMeta, SupportsStageDispatch,
};

use crate::result::AnalysisResult;
use crate::{
    AbstractValue, CallSemantics, Continuation, Interpretable, Interpreter, InterpreterError,
    StageAccess,
};

use super::interp::AbstractInterpreter;

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + 'ir,
    G: 'ir,
{
    /// Runtime-dispatched analysis entrypoint.
    pub fn analyze(
        &mut self,
        callee: SpecializedFunction,
        stage: CompileStage,
        args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        for<'a> S:
            SupportsStageDispatch<AnalyzeDynAction<'a, 'ir, V, S, E, G>, AnalysisResult<V>, E>,
    {
        self.call_handler = Some(Self::analyze);
        let pipeline = self.pipeline;
        let mut action = AnalyzeDynAction {
            interp: self,
            callee,
            args,
        };
        crate::dispatch::dispatch_in_pipeline(pipeline, stage, &mut action)
    }

    pub(crate) fn analyze_with_stage_id<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect
            + Interpretable<'ir, Self, L>
            + CallSemantics<'ir, Self, L, Result = AnalysisResult<V>>
            + 'ir,
    {
        let stage = self.resolve_stage_info::<L>(stage_id)?;
        self.analyze_in_resolved_stage::<L>(callee, stage_id, stage, args)
    }

    fn analyze_in_resolved_stage<L>(
        &mut self,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        stage: &'ir StageInfo<L>,
        args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        L: Dialect
            + Interpretable<'ir, Self, L>
            + CallSemantics<'ir, Self, L, Result = AnalysisResult<V>>
            + 'ir,
    {
        let key = Self::summary_key(stage_id, callee);

        if let Some(result) = self.summaries.get(&key).and_then(|c| c.lookup(args)) {
            return Ok(result.clone());
        }

        let is_recursive = self
            .frames
            .iter()
            .any(|f| f.callee() == callee && f.stage() == stage_id);
        if is_recursive {
            let result = self
                .summaries
                .get(&key)
                .and_then(|c| c.tentative_result())
                .cloned()
                .unwrap_or_else(AnalysisResult::bottom);
            return Ok(result);
        }

        let spec = callee
            .get_info(stage)
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: crate::StageResolutionError::MissingCallee { callee },
            })?;
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);
        def.eval_call(self, stage, callee, args)
    }

    /// Run forward abstract interpretation starting from `entry` block with
    /// `initial_args` bound to the block's arguments.
    ///
    /// Returns an [`AnalysisResult`] containing all SSA values and the joined
    /// return value.
    pub fn run_forward<L>(
        &mut self,
        stage_id: CompileStage,
        entry: Block,
        initial_args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage = self.resolve_stage_info::<L>(stage_id)?;

        {
            let block_info = entry.expect_info(stage);
            let arg_ssas: Vec<SSAValue> = block_info
                .arguments
                .iter()
                .map(|ba| SSAValue::from(*ba))
                .collect();
            if arg_ssas.len() != initial_args.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: arg_ssas.len(),
                    got: initial_args.len(),
                }
                .into());
            }
            let frame = self.frames.current_mut()?;
            let (values, fp) = frame.values_and_extra_mut();
            for (ssa, val) in arg_ssas.iter().zip(initial_args.iter()) {
                values.insert(*ssa, val.clone());
            }
            fp.block_args.insert(entry, arg_ssas);
            fp.worklist.push_unique(entry);
        }

        let mut return_value: Option<V> = None;
        let mut iterations = 0;

        loop {
            let block = {
                let fp = self.frames.current_mut()?.extra_mut();
                fp.worklist.pop()
            };
            let Some(block) = block else { break };

            iterations += 1;
            if iterations > self.max_iterations {
                return Err(InterpreterError::FuelExhausted.into());
            }

            let control = self.eval_block(stage, block)?;
            self.propagate_control::<L>(stage, &control, false, &mut return_value)?;
        }

        if self.narrowing_iterations > 0 {
            let blocks: Vec<Block> = self
                .frames
                .current()?
                .extra()
                .block_args
                .keys()
                .copied()
                .collect();
            for _ in 0..self.narrowing_iterations {
                let mut changed = false;
                for &block in &blocks {
                    let control = self.eval_block(stage, block)?;
                    changed |=
                        self.propagate_control::<L>(stage, &control, true, &mut return_value)?;
                }
                if !changed {
                    break;
                }
            }
        }

        let frame = self.frames.current()?;
        Ok(AnalysisResult::new(
            frame.values().clone(),
            frame.extra().block_args.clone(),
            return_value,
        ))
    }

    // -- Internal helpers ---------------------------------------------------

    /// Set the tentative summary for `(stage, callee)`.
    pub(crate) fn set_tentative(
        &mut self,
        stage: CompileStage,
        callee: SpecializedFunction,
        args: &[V],
        result: AnalysisResult<V>,
    ) {
        self.cache_mut(stage, callee)
            .set_tentative(args.to_vec(), result);
    }

    /// Get the full tentative analysis result for `(stage, callee)`.
    pub(crate) fn tentative_result(
        &self,
        stage: CompileStage,
        callee: SpecializedFunction,
    ) -> Option<&AnalysisResult<V>> {
        self.summaries
            .get(&Self::summary_key(stage, callee))
            .and_then(|c| c.tentative_result())
    }

    /// Promote the tentative summary to a computed entry.
    pub(crate) fn promote_tentative(
        &mut self,
        stage: CompileStage,
        callee: SpecializedFunction,
        args: &[V],
        result: AnalysisResult<V>,
    ) {
        self.cache_mut(stage, callee)
            .promote_tentative(args.to_vec(), result);
    }

    /// Return the maximum number of summary iterations.
    pub(crate) fn max_summary_iterations(&self) -> usize {
        self.max_summary_iterations
    }

    /// Handle control flow edge propagation for both widening and narrowing.
    ///
    /// During widening (`narrowing=false`), changed targets are enqueued to
    /// the worklist. Returns whether any edge changed.
    fn propagate_control<L>(
        &mut self,
        stage: &'ir StageInfo<L>,
        control: &Continuation<V>,
        narrowing: bool,
        return_value: &mut Option<V>,
    ) -> Result<bool, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + 'ir,
    {
        let mut changed = false;
        match control {
            Continuation::Jump(succ, args) => {
                changed |= self.propagate_edge::<L>(stage, succ.target(), args, narrowing)?;
            }
            Continuation::Fork(targets) => {
                for (succ, args) in targets {
                    changed |= self.propagate_edge::<L>(stage, succ.target(), args, narrowing)?;
                }
            }
            Continuation::Return(v) | Continuation::Yield(v) => {
                *return_value = Some(match return_value.take() {
                    Some(existing) => {
                        if narrowing {
                            existing.narrow(v)
                        } else {
                            existing.join(v)
                        }
                    }
                    None => v.clone(),
                });
            }
            Continuation::Continue | Continuation::Call { .. } => {}
            Continuation::Ext(inf) => match *inf {},
        }
        Ok(changed)
    }

    /// Propagate a single control flow edge and enqueue the target if changed.
    fn propagate_edge<L>(
        &mut self,
        stage: &'ir StageInfo<L>,
        target: Block,
        args: &[V],
        narrowing: bool,
    ) -> Result<bool, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + 'ir,
    {
        if self.propagate_block_args::<L>(stage, target, args, narrowing)? {
            if !narrowing {
                let fp = self.frames.current_mut()?.extra_mut();
                fp.worklist.push_unique(target);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Propagate block argument values to the target block. Only block
    /// argument SSA values are joined/widened — all other SSA values in
    /// the frame are write-once and shared across all paths.
    ///
    /// Returns `true` if the target's block arg state changed (or first visit).
    fn propagate_block_args<L>(
        &mut self,
        stage: &'ir StageInfo<L>,
        target: Block,
        args: &[V],
        narrowing: bool,
    ) -> Result<bool, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + 'ir,
    {
        let target_arg_ssas: Vec<SSAValue> = {
            let block_info = target.expect_info(stage);
            block_info
                .arguments
                .iter()
                .map(|ba| SSAValue::from(*ba))
                .collect()
        };

        if target_arg_ssas.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: target_arg_ssas.len(),
                got: args.len(),
            }
            .into());
        }

        let widening_strategy = self.widening_strategy;
        let frame = self.frames.current_mut()?;
        let (values, fp) = frame.values_and_extra_mut();

        let first_visit = !fp.block_args.contains_key(&target);

        if first_visit {
            for (ssa, val) in target_arg_ssas.iter().zip(args.iter()) {
                values.insert(*ssa, val.clone());
            }
            fp.block_args.insert(target, target_arg_ssas);
            Ok(true)
        } else {
            let visit_count = fp.visit_counts.entry(target).or_insert(0);
            *visit_count += 1;
            let current_count = *visit_count;

            let mut changed = false;
            for (ssa, new_val) in target_arg_ssas.iter().zip(args.iter()) {
                match values.entry(*ssa) {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        let merged = if narrowing {
                            entry.get().narrow(new_val)
                        } else {
                            widening_strategy.merge(entry.get(), new_val, current_count)
                        };
                        if !merged.is_subseteq(entry.get()) || !entry.get().is_subseteq(&merged) {
                            changed = true;
                        }
                        *entry.get_mut() = merged;
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(new_val.clone());
                        changed = true;
                    }
                }
            }
            Ok(changed)
        }
    }
}

#[doc(hidden)]
pub struct AnalyzeDynAction<'a, 'ir, V, S, E, G>
where
    S: StageMeta,
{
    interp: &'a mut AbstractInterpreter<'ir, V, S, E, G>,
    callee: SpecializedFunction,
    args: &'a [V],
}

impl<'a, 'ir, V, S, E, G, L> StageAction<S, L> for AnalyzeDynAction<'a, 'ir, V, S, E, G>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: StageMeta + HasStageInfo<L> + 'ir,
    G: 'ir,
    L: Dialect
        + Interpretable<'ir, AbstractInterpreter<'ir, V, S, E, G>, L>
        + CallSemantics<'ir, AbstractInterpreter<'ir, V, S, E, G>, L, Result = AnalysisResult<V>>
        + 'ir,
{
    type Output = AnalysisResult<V>;
    type Error = E;

    fn run(
        &mut self,
        stage_id: CompileStage,
        _stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        self.interp
            .analyze_with_stage_id::<L>(self.callee, stage_id, self.args)
    }
}
