use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, SSAValue, SpecializedFunction,
    StageAction, StageInfo, StageMeta, SupportsStageDispatch,
};
use smallvec::SmallVec;

use crate::result::AnalysisResult;
use crate::{
    AbstractValue, BlockEvaluator, CallSemantics, Continuation, Interpretable, InterpreterError,
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
            + Interpretable<'ir, Self>
            + CallSemantics<'ir, Self, Result = AnalysisResult<V>>
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
        S: HasStageInfo<L>,
        L: Dialect
            + Interpretable<'ir, Self>
            + CallSemantics<'ir, Self, Result = AnalysisResult<V>>
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
        def.eval_call::<L>(self, stage, callee, args)
    }

    /// Run forward abstract interpretation starting from `entry` block with
    /// `initial_args` bound to the block's arguments.
    ///
    /// On success the current frame is consumed (popped) and its state is
    /// moved into the returned [`AnalysisResult`]. The caller must pop the
    /// frame itself on error paths.
    pub fn run_forward<L>(
        &mut self,
        stage_id: CompileStage,
        entry: Block,
        initial_args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self> + 'ir,
    {
        let stage = self.resolve_stage_info::<L>(stage_id)?;

        {
            let block_info = entry.expect_info(stage);
            if block_info.arguments.len() != initial_args.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: block_info.arguments.len(),
                    got: initial_args.len(),
                }
                .into());
            }
            let frame = self.frames.current_mut()?;
            let (values, fp) = frame.values_and_extra_mut();
            let arg_ssas: Vec<SSAValue> = block_info
                .arguments
                .iter()
                .zip(initial_args.iter())
                .map(|(ba, val)| {
                    let ssa = SSAValue::from(*ba);
                    values.insert(ssa, val.clone());
                    ssa
                })
                .collect();
            fp.block_args.insert(entry, arg_ssas);
            fp.worklist.push_unique(entry);
        }

        let mut return_values: Option<SmallVec<[V; 1]>> = None;
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
            self.propagate_control::<L>(stage, &control, false, &mut return_values)?;
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
                        self.propagate_control::<L>(stage, &control, true, &mut return_values)?;
                }
                if !changed {
                    break;
                }
            }
        }

        let frame = self.frames.pop()?;
        let (_callee, _stage, values, fp) = frame.into_parts();
        Ok(AnalysisResult::new(values, fp.block_args, return_values))
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
        return_values: &mut Option<SmallVec<[V; 1]>>,
    ) -> Result<bool, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + 'ir,
    {
        let mut changed = false;
        match control {
            Continuation::Jump(block, args) => {
                changed |= self.propagate_edge::<L>(stage, *block, args, narrowing)?;
            }
            Continuation::Fork(targets) => {
                for (block, args) in targets {
                    changed |= self.propagate_edge::<L>(stage, *block, args, narrowing)?;
                }
            }
            Continuation::Return(values) | Continuation::Yield(values) => {
                match (&mut *return_values, values) {
                    (None, vs) => *return_values = Some(vs.clone()),
                    (Some(existing), vs) if existing.len() != vs.len() => {
                        return Err(InterpreterError::ArityMismatch {
                            expected: existing.len(),
                            got: vs.len(),
                        }
                        .into());
                    }
                    (Some(existing), vs) => {
                        for (e, v) in existing.iter_mut().zip(vs.iter()) {
                            *e = if narrowing { e.narrow(v) } else { e.join(v) };
                        }
                    }
                }
            }
            // Call is handled inline in `eval_block` (the call handler writes
            // the return values directly), so it never reaches propagation.
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
        let block_info = target.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }

        let widening_strategy = self.widening_strategy;
        let frame = self.frames.current_mut()?;
        let (values, fp) = frame.values_and_extra_mut();

        let first_visit = !fp.block_args.contains_key(&target);

        if first_visit {
            let arg_ssas: Vec<SSAValue> = block_info
                .arguments
                .iter()
                .zip(args.iter())
                .map(|(ba, val)| {
                    let ssa = SSAValue::from(*ba);
                    values.insert(ssa, val.clone());
                    ssa
                })
                .collect();
            fp.block_args.insert(target, arg_ssas);
            Ok(true)
        } else {
            let visit_count = fp.visit_counts.entry(target).or_insert(0);
            *visit_count += 1;
            let current_count = *visit_count;

            let mut changed = false;
            for (ba, new_val) in block_info.arguments.iter().zip(args.iter()) {
                let ssa = SSAValue::from(*ba);
                match values.entry(ssa) {
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
        + Interpretable<'ir, AbstractInterpreter<'ir, V, S, E, G>>
        + CallSemantics<'ir, AbstractInterpreter<'ir, V, S, E, G>, Result = AnalysisResult<V>>
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
