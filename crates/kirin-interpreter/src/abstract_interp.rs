use std::collections::VecDeque;
use std::marker::PhantomData;

use fxhash::FxHashMap;
use kirin_ir::{
    Block, CompileStage, CompileStageInfo, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue,
    SSAValue, SpecializedFunction, StageInfo,
};

use crate::result::AnalysisResult;
use crate::widening::WideningStrategy;
use crate::{
    AbstractContinuation, AbstractValue, Continuation, Frame, Interpretable, Interpreter,
    InterpreterError,
};

/// Per-function fixpoint state stored as frame extra data.
///
/// Block argument SSA value IDs are tracked here; the actual SSA values
/// (both block args and statement results) live in [`Frame::values`].
#[derive(Debug, Default)]
pub(crate) struct FixpointState {
    worklist: VecDeque<Block>,
    /// Per-block argument SSA value IDs. Key presence = block visited.
    block_args: FxHashMap<Block, Vec<SSAValue>>,
    /// Per-block visit counts for [`WideningStrategy::Delayed`].
    visit_counts: FxHashMap<Block, usize>,
}

/// A single context-sensitive summary entry in the cache.
#[derive(Debug, Clone)]
pub struct SummaryEntry<V> {
    /// Argument abstract values this entry was computed for.
    pub args: Vec<V>,
    /// The analysis result for this context.
    pub result: AnalysisResult<V>,
    /// Whether this entry has been invalidated. Invalidated entries are
    /// skipped during lookup but retained until garbage-collected.
    pub invalidated: bool,
}

/// Per-function summary cache supporting multiple call contexts.
///
/// Each function may have:
/// - An optional **fixed** summary that is always returned and never
///   overwritten by analysis.
/// - Zero or more **computed** entries, each for a different call context
///   (argument abstract values). Lookup finds the tightest (most specific)
///   non-invalidated entry whose args subsume the query.
/// - At most one **tentative** entry used during recursive fixpoint iteration.
#[derive(Debug, Clone)]
pub struct SummaryCache<V> {
    /// User-provided fixed summary. Not subject to invalidation.
    fixed: Option<AnalysisResult<V>>,
    /// Computed and seed entries, possibly for multiple call contexts.
    ///
    /// Lookup is a linear scan (`find_best_match`). This is fine for the
    /// expected cardinality (single-digit contexts per function). If profiling
    /// shows this is hot, consider a lattice-height–based index — note that
    /// `BTreeMap` does not apply here because `Vec<V>` under subsumption is a
    /// partial order, not a total order.
    entries: Vec<SummaryEntry<V>>,
    /// Tentative entry during recursive fixpoint (at most one active).
    tentative: Option<SummaryEntry<V>>,
}

impl<V> Default for SummaryCache<V> {
    fn default() -> Self {
        Self {
            fixed: None,
            entries: Vec::new(),
            tentative: None,
        }
    }
}

impl<V: AbstractValue + Clone> SummaryCache<V> {
    /// Find the tightest non-invalidated entry whose args subsume `query_args`.
    ///
    /// "Tightest" means: among all matching entries, the one whose args are
    /// pointwise subsumed by every other match (i.e. the most specific).
    fn find_best_match(&self, query_args: &[V]) -> Option<&SummaryEntry<V>> {
        let mut best: Option<&SummaryEntry<V>> = None;
        for entry in &self.entries {
            if entry.invalidated {
                continue;
            }
            if entry.args.len() != query_args.len() {
                continue;
            }
            let subsumes = query_args
                .iter()
                .zip(entry.args.iter())
                .all(|(q, cached)| q.is_subseteq(cached));
            if !subsumes {
                continue;
            }
            best = Some(match best {
                None => entry,
                Some(current) => {
                    // Pick the entry with tighter (more specific) args
                    let tighter = entry
                        .args
                        .iter()
                        .zip(current.args.iter())
                        .all(|(e, b)| e.is_subseteq(b));
                    if tighter { entry } else { current }
                }
            });
        }
        best
    }

    /// Get the tentative result (for recursive fixpoint), if any.
    fn tentative_result(&self) -> Option<&AnalysisResult<V>> {
        self.tentative.as_ref().map(|t| &t.result)
    }
}

/// Worklist-based abstract interpreter for fixpoint computation.
///
/// Unlike [`crate::StackInterpreter`] which follows a single concrete execution
/// path, `AbstractInterpreter` explores all reachable paths by joining abstract
/// states at block entry points and iterating until a fixpoint is reached.
///
/// Widening is applied at join points to guarantee termination for infinite
/// abstract domains.
pub struct AbstractInterpreter<'ir, V, S, E = InterpreterError, G = ()>
where
    S: CompileStageInfo,
{
    pipeline: &'ir Pipeline<S>,
    active_stage: CompileStage,
    global: G,
    frames: Vec<Frame<V, FixpointState>>,
    widening_strategy: WideningStrategy,
    max_iterations: usize,
    narrowing_iterations: usize,
    summaries: FxHashMap<SpecializedFunction, SummaryCache<V>>,
    max_depth: Option<usize>,
    max_summary_iterations: usize,
    _error: PhantomData<E>,
}

/// Builder for inserting function summaries into an [`AbstractInterpreter`].
///
/// Obtained via [`AbstractInterpreter::insert_summary`].
pub struct SummaryInserter<'a, 'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    interp: &'a mut AbstractInterpreter<'ir, V, S, E, G>,
    callee: SpecializedFunction,
}

impl<V: Clone, S: CompileStageInfo, E, G> SummaryInserter<'_, '_, V, S, E, G> {
    /// Insert an immutable summary. Analysis will never re-analyze this function.
    pub fn fixed(self, result: AnalysisResult<V>) {
        self.interp
            .summaries
            .entry(self.callee)
            .or_default()
            .fixed = Some(result);
    }

    /// Insert a refinable seed. Analysis may improve upon this summary.
    pub fn seed(self, args: Vec<V>, result: AnalysisResult<V>) {
        self.interp
            .summaries
            .entry(self.callee)
            .or_default()
            .entries
            .push(SummaryEntry {
                args,
                result,
                invalidated: false,
            });
    }
}

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> AbstractInterpreter<'ir, V, S, E, ()>
where
    S: CompileStageInfo,
{
    pub fn new(pipeline: &'ir Pipeline<S>, active_stage: CompileStage) -> Self {
        Self {
            pipeline,
            active_stage,
            global: (),
            widening_strategy: WideningStrategy::AllJoins,
            max_iterations: 1000,
            narrowing_iterations: 3,
            frames: Vec::new(),
            summaries: FxHashMap::default(),
            max_depth: None,
            max_summary_iterations: 100,
            _error: PhantomData,
        }
    }

    /// Attach global state, transforming `G` from `()` to the provided type.
    pub fn with_global<G>(self, global: G) -> AbstractInterpreter<'ir, V, S, E, G> {
        AbstractInterpreter {
            pipeline: self.pipeline,
            active_stage: self.active_stage,
            global,
            widening_strategy: self.widening_strategy,
            max_iterations: self.max_iterations,
            narrowing_iterations: self.narrowing_iterations,
            frames: self.frames,
            summaries: self.summaries,
            max_depth: self.max_depth,
            max_summary_iterations: self.max_summary_iterations,
            _error: PhantomData,
        }
    }
}

// -- Builder methods --------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn with_widening(mut self, strategy: WideningStrategy) -> Self {
        self.widening_strategy = strategy;
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_narrowing_iterations(mut self, n: usize) -> Self {
        self.narrowing_iterations = n;
        self
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn with_max_summary_iterations(mut self, n: usize) -> Self {
        self.max_summary_iterations = n;
        self
    }
}

// -- Accessors --------------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn global(&self) -> &G {
        &self.global
    }

    pub fn global_mut(&mut self) -> &mut G {
        &mut self.global
    }

    /// Look up the best cached summary for `callee` given `args`.
    ///
    /// Returns the fixed summary if one exists, otherwise finds the tightest
    /// non-invalidated entry whose cached args subsume the query.
    pub fn summary(&self, callee: SpecializedFunction, args: &[V]) -> Option<&AnalysisResult<V>>
    where
        V: AbstractValue + Clone,
    {
        let cache = self.summaries.get(&callee)?;
        if let Some(ref fixed) = cache.fixed {
            return Some(fixed);
        }
        cache.find_best_match(args).map(|e| &e.result)
    }

    /// Look up the full summary cache for `callee`.
    pub fn summary_cache(&self, callee: SpecializedFunction) -> Option<&SummaryCache<V>> {
        self.summaries.get(&callee)
    }

    /// Return a builder for inserting a function summary.
    pub fn insert_summary(
        &mut self,
        callee: SpecializedFunction,
    ) -> SummaryInserter<'_, 'ir, V, S, E, G> {
        SummaryInserter {
            interp: self,
            callee,
        }
    }

    /// Mark all computed entries for `callee` as invalidated so the next
    /// [`analyze`](Self::analyze) call re-runs the analysis. Invalidated
    /// entries are retained (for inspection) until
    /// [`gc_summaries`](Self::gc_summaries) is called.
    ///
    /// User-fixed summaries are **not** affected.
    ///
    /// Returns the number of entries invalidated.
    pub fn invalidate_summary(&mut self, callee: SpecializedFunction) -> usize {
        let Some(cache) = self.summaries.get_mut(&callee) else {
            return 0;
        };
        let mut count = 0;
        for entry in &mut cache.entries {
            if !entry.invalidated {
                entry.invalidated = true;
                count += 1;
            }
        }
        cache.tentative = None;
        count
    }

    /// Remove invalidated entries across all functions, freeing memory.
    pub fn gc_summaries(&mut self) {
        for cache in self.summaries.values_mut() {
            cache.entries.retain(|e| !e.invalidated);
        }
        self.summaries.retain(|_, cache| {
            cache.fixed.is_some() || !cache.entries.is_empty() || cache.tentative.is_some()
        });
    }

    /// Unconditionally remove all summaries (including user-fixed) for `callee`.
    ///
    /// Returns `true` if a cache entry was present.
    pub fn remove_summary(&mut self, callee: SpecializedFunction) -> bool {
        self.summaries.remove(&callee).is_some()
    }
}

// -- Interpreter trait impl -------------------------------------------------

impl<'ir, V, S, E, G> Interpreter for AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo,
{
    type Value = V;
    type Error = E;
    type Ext = std::convert::Infallible;
    type StageInfo = S;

    fn read_ref(&self, value: SSAValue) -> Result<&V, E> {
        self.frames
            .last()
            .and_then(|f| f.read(value))
            .ok_or_else(|| InterpreterError::UnboundValue(value).into())
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.frames
            .last_mut()
            .ok_or_else(|| InterpreterError::NoFrame.into())?
            .write(result, value);
        Ok(())
    }

    fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    fn active_stage(&self) -> CompileStage {
        self.active_stage
    }
}

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone,
    E: From<InterpreterError>,
    S: CompileStageInfo,
{
    /// Resolve the entry block of a specialized function.
    fn resolve_entry_block<L>(&self, callee: SpecializedFunction) -> Option<Block>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.resolve_stage::<L>();
        let spec = callee.expect_info(stage);
        let body_stmt = *spec.body();
        let region = body_stmt.regions::<L>(stage).next()?;
        region.blocks(stage).next()
    }

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
        L: Dialect + Interpretable<Self>,
    {
        // 1. UserFixed summaries are always returned as-is
        if let Some(cache) = self.summaries.get(&callee) {
            if let Some(ref fixed) = cache.fixed {
                return Ok(fixed.clone());
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

        // 5. Resolve entry block
        let entry = self
            .resolve_entry_block::<L>(callee)
            .ok_or_else(|| InterpreterError::MissingEntry.into())?;

        // 6. Insert tentative summary before pushing frame
        self.summaries.entry(callee).or_default().tentative = Some(SummaryEntry {
            args: args.to_vec(),
            result: AnalysisResult::bottom(),
            invalidated: false,
        });

        // 7. Outer fixpoint loop for inter-procedural convergence
        let mut summary_iterations = 0;
        let final_result = loop {
            summary_iterations += 1;
            if summary_iterations > self.max_summary_iterations {
                return Err(InterpreterError::FuelExhausted.into());
            }

            // Push frame and run forward analysis
            self.frames
                .push(Frame::new(callee, FixpointState::default()));
            let result = self.run_forward::<L>(entry, args);
            self.frames.pop().expect("frame stack underflow");

            let result = result?;

            // Check if summary changed
            let old_return = self
                .summaries
                .get(&callee)
                .and_then(|c| c.tentative_result())
                .and_then(|r| r.return_value().cloned());
            let new_return = result.return_value().cloned();

            // Update tentative summary
            self.summaries.entry(callee).or_default().tentative = Some(SummaryEntry {
                args: args.to_vec(),
                result: result.clone(),
                invalidated: false,
            });

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

        // 8. Promote tentative to computed entry
        let cache = self.summaries.entry(callee).or_default();
        cache.tentative = None;
        cache.entries.push(SummaryEntry {
            args: args.to_vec(),
            result: final_result.clone(),
            invalidated: false,
        });

        Ok(final_result)
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
        L: Dialect + Interpretable<Self>,
    {
        // 1. Seed entry block
        {
            let stage = self.resolve_stage::<L>();
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

            let control = self.interpret_block::<L>(block)?;
            self.propagate_control::<L>(&control, false, &mut return_value)?;
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
                    let control = self.interpret_block::<L>(block)?;
                    changed |= self.propagate_control::<L>(&control, true, &mut return_value)?;
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

    fn resolve_stage<L>(&self) -> &'ir StageInfo<L>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        self.pipeline
            .stage(self.active_stage)
            .and_then(|s| s.try_stage_info())
            .expect("active stage does not contain StageInfo for this dialect")
    }

    /// Handle control flow edge propagation for both widening and narrowing.
    ///
    /// During widening (`narrowing=false`), changed targets are enqueued to
    /// the worklist. Returns whether any edge changed.
    fn propagate_control<L>(
        &mut self,
        control: &AbstractContinuation<V>,
        narrowing: bool,
        return_value: &mut Option<V>,
    ) -> Result<bool, E>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let mut changed = false;
        match control {
            Continuation::Jump(target, args) => {
                changed |= self.propagate_edge::<L>(*target, args, narrowing)?;
            }
            Continuation::Fork(targets) => {
                for (target, args) in targets {
                    changed |= self.propagate_edge::<L>(*target, args, narrowing)?;
                }
            }
            Continuation::Return(v) => {
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
    fn propagate_edge<L>(&mut self, target: Block, args: &[V], narrowing: bool) -> Result<bool, E>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        if self.propagate_block_args::<L>(target, args, narrowing)? {
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

    /// Interpret all statements in a block sequentially, returning the
    /// final control action from the terminator.
    fn interpret_block<L>(&mut self, block: Block) -> Result<AbstractContinuation<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        // Collect statement IDs and terminator up front (cheap Copy)
        // to avoid holding a borrow on stage across interpret calls.
        let (stmts, terminator) = {
            let stage = self.resolve_stage::<L>();
            let stmts: Vec<_> = block.statements(stage).collect();
            let terminator = block.terminator(stage);
            (stmts, terminator)
        };

        // Interpret each statement
        for stmt in stmts {
            let control = {
                let stage = self.resolve_stage::<L>();
                let def: &L = stmt.definition(stage);
                def.interpret(self)?
            };
            match control {
                Continuation::Continue => {}
                Continuation::Call {
                    callee,
                    args,
                    result,
                } => {
                    let analysis = self.analyze::<L>(callee, &args)?;
                    let return_val = analysis.return_value().cloned().unwrap_or_else(V::bottom);
                    self.write(result, return_val)?;
                }
                other => return Ok(other),
            }
        }

        // Interpret the terminator
        if let Some(term) = terminator {
            let control = {
                let stage = self.resolve_stage::<L>();
                let def: &L = term.definition(stage);
                def.interpret(self)?
            };
            Ok(control)
        } else {
            Err(InterpreterError::MissingEntry.into())
        }
    }

    /// Propagate block argument values to the target block. Only block
    /// argument SSA values are joined/widened — all other SSA values in
    /// the frame are write-once and shared across all paths.
    ///
    /// Returns `true` if the target's block arg state changed (or first visit).
    fn propagate_block_args<L>(
        &mut self,
        target: Block,
        args: &[V],
        narrowing: bool,
    ) -> Result<bool, E>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let target_arg_ssas: Vec<SSAValue> = {
            let stage = self.resolve_stage::<L>();
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
