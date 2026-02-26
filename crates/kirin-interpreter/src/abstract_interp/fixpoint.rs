use kirin_ir::{
    Block, CompileStageInfo, Dialect, GetInfo, HasStageInfo, SSAValue, SpecializedFunction,
};

use super::FixpointState;
use crate::result::AnalysisResult;
use crate::{
    AbstractContinuation, AbstractValue, EvalBlock, EvalCall, Continuation, Frame,
    Interpretable, Interpreter, InterpreterError,
};

use super::interp::AbstractInterpreter;

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone + 'ir,
    E: From<InterpreterError> + 'ir,
    S: CompileStageInfo + 'ir,
    G: 'ir,
{
    /// Analyze a function, returning its [`AnalysisResult`].
    ///
    /// Results are cached per `(callee, args)`: a cached entry is reused only
    /// when every new argument is subsumed by the corresponding cached argument
    /// (`new_arg ⊑ cached_arg`). This ensures context-sensitive soundness —
    /// calls with more precise arguments trigger a fresh analysis.
    ///
    /// Recursive calls are handled via tentative summaries and an
    /// inter-procedural fixpoint loop. When a callee is already on the frame
    /// stack, its current tentative summary (or `bottom` if none) is returned
    /// immediately. The outermost call drives re-analysis until all summaries
    /// stabilize.
    pub fn analyze<L>(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect
            + Interpretable<'ir, Self, L>
            + EvalCall<'ir, Self, L, Result = AnalysisResult<V>>
            + 'ir,
    {
        // 1. UserFixed summaries are always returned as-is
        if let Some(cache) = self.summaries.get(&callee) {
            if let Some(ref fixed) = cache.fixed() {
                return Ok((*fixed).clone());
            }
        }

        // 2. Check computed/seed cache — find tightest non-invalidated match
        if let Some(cache) = self.summaries.get(&callee) {
            if let Some(entry) = cache.find_best_match(args) {
                return Ok(entry.result.clone());
            }
        }

        // 3. Check for recursion (callee already on frame stack)
        let is_recursive = self.frames.iter().any(|f| f.callee() == callee);
        if is_recursive {
            // Return tentative summary (bottom if none exists yet)
            let result = self
                .summaries
                .get(&callee)
                .and_then(|c| c.tentative_result())
                .cloned()
                .unwrap_or_else(AnalysisResult::bottom);
            return Ok(result);
        }

        // 4. Depth check
        if let Some(max) = self.max_depth {
            if self.frames.len() >= max {
                return Err(InterpreterError::MaxDepthExceeded.into());
            }
        }

        // 5. Install call handler so eval_block can dispatch nested calls
        self.call_handler = Some(Self::analyze::<L>);

        // 6. Delegate to EvalCall
        let stage = self.active_stage_info::<L>();
        let spec = callee.expect_info(stage);
        let body_stmt = *spec.body();
        let def: &L = body_stmt.definition(stage);
        def.eval_call(self, callee, args)
    }

    /// Run forward abstract interpretation starting from `entry` block with
    /// `initial_args` bound to the block's arguments.
    ///
    /// Returns an [`AnalysisResult`] containing all SSA values and the joined
    /// return value.
    pub fn run_forward<L>(
        &mut self,
        entry: Block,
        initial_args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<'ir, Self, L> + 'ir,
    {
        let stage = self.active_stage_info::<L>();

        // 1. Seed entry block
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
            let frame = self
                .frames
                .last_mut()
                .ok_or_else(|| InterpreterError::NoFrame.into())?;
            let (values, fp) = frame.values_and_extra_mut();
            for (ssa, val) in arg_ssas.iter().zip(initial_args.iter()) {
                values.insert(*ssa, val.clone());
            }
            fp.block_args.insert(entry, arg_ssas);
            fp.worklist.push_back(entry);
        }

        let mut return_value: Option<V> = None;
        let mut iterations = 0;

        // 2. Widening fixpoint loop
        loop {
            let block = {
                let fp = self
                    .frames
                    .last_mut()
                    .ok_or_else(|| InterpreterError::NoFrame.into())?
                    .extra_mut();
                fp.worklist.pop_front()
            };
            let Some(block) = block else { break };

            iterations += 1;
            if iterations > self.max_iterations {
                return Err(InterpreterError::FuelExhausted.into());
            }

            let control = EvalBlock::<'ir, L>::eval_block(self, stage, block)?;
            self.propagate_control::<L>(stage, &control, false, &mut return_value)?;
        }

        // 3. Narrowing phase
        if self.narrowing_iterations > 0 {
            let blocks: Vec<Block> = self
                .frames
                .last()
                .ok_or_else(|| InterpreterError::NoFrame.into())?
                .extra()
                .block_args
                .keys()
                .copied()
                .collect();
            for _ in 0..self.narrowing_iterations {
                let mut changed = false;
                for &block in &blocks {
                    let control = EvalBlock::<'ir, L>::eval_block(self, stage, block)?;
                    changed |=
                        self.propagate_control::<L>(stage, &control, true, &mut return_value)?;
                }
                if !changed {
                    break;
                }
            }
        }

        let frame = self
            .frames
            .last()
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
        Ok(AnalysisResult::new(
            frame.values().clone(),
            frame.extra().block_args.clone(),
            return_value,
        ))
    }

    // -- Internal helpers ---------------------------------------------------

    /// Push a new analysis frame for `callee`.
    pub(crate) fn push_analysis_frame(&mut self, callee: SpecializedFunction) {
        self.frames
            .push(Frame::new(callee, FixpointState::default()));
    }

    /// Pop the current analysis frame. Panics if no frame exists.
    pub(crate) fn pop_analysis_frame(&mut self) {
        self.frames.pop().expect("frame stack underflow");
    }

    /// Set the tentative summary for `callee`.
    pub(crate) fn set_tentative(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
        result: AnalysisResult<V>,
    ) {
        self.summaries
            .entry(callee)
            .or_default()
            .set_tentative(args.to_vec(), result);
    }

    /// Get the full tentative analysis result for `callee`.
    pub(crate) fn tentative_result(
        &self,
        callee: SpecializedFunction,
    ) -> Option<&AnalysisResult<V>> {
        self.summaries
            .get(&callee)
            .and_then(|c| c.tentative_result())
    }

    /// Promote the tentative summary to a computed entry.
    pub(crate) fn promote_tentative(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
        result: AnalysisResult<V>,
    ) {
        self.summaries
            .entry(callee)
            .or_default()
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
        stage: &'ir kirin_ir::StageInfo<L>,
        control: &AbstractContinuation<V>,
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
        stage: &'ir kirin_ir::StageInfo<L>,
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
                let fp = self
                    .frames
                    .last_mut()
                    .ok_or_else(|| InterpreterError::NoFrame.into())?
                    .extra_mut();
                if !fp.worklist.contains(&target) {
                    fp.worklist.push_back(target);
                }
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
        stage: &'ir kirin_ir::StageInfo<L>,
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
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())?;
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
