use std::collections::VecDeque;
use std::marker::PhantomData;

use kirin_interpreter::{AbstractValue, WideningStrategy};
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta, Symbol,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::abstract_domain::BaseDomain;
use crate::core::Core;
use crate::env::{Env, Interpretable};
use crate::error::InterpreterError;

// ---------------------------------------------------------------------------
// Intraprocedural state — per-function block-level analysis
// ---------------------------------------------------------------------------

struct FuncState<V> {
    /// Abstract values at each block's entry point.
    block_in: FxHashMap<Block, Vec<V>>,
    /// Per-block visit counts for the widening threshold.
    visit_counts: FxHashMap<Block, usize>,
    /// Blocks pending re-analysis within this function.
    block_worklist: VecDeque<Block>,
}

impl<V> FuncState<V> {
    fn new() -> Self {
        Self {
            block_in: FxHashMap::default(),
            visit_counts: FxHashMap::default(),
            block_worklist: VecDeque::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Interprocedural summary — call-graph–level fixpoint state
// ---------------------------------------------------------------------------

struct FuncSummary<V> {
    /// Joined abstract inputs seen across all call sites.
    input: Vec<V>,
    /// Abstract return value (None = bottom, not yet analyzed).
    output: Option<V>,
    /// Cached entry block so we don't re-resolve on every re-analysis.
    entry_block: Block,
}

// ---------------------------------------------------------------------------
// AbstractInterp — summary-based interprocedural fixpoint interpreter
//
// Correctness properties:
//
// 1. **No Rust-stack recursion.** Calls are handled by looking up (or
//    initialising) the callee's summary and returning the current output.
//    The callee is queued for (re-)analysis in `func_worklist`; the outer
//    loop drives everything.
//
// 2. **Cycle detection / mutual recursion.** Because every function is
//    represented by a summary keyed on `SpecializedFunction`, re-entrant
//    calls return the current (possibly bottom) summary output instead of
//    looping.  The outer fixpoint re-queues callers whenever a callee's
//    output improves, propagating precision upward monotonically.
//
// 3. **Interprocedural memoization.** If two call sites pass the same (or
//    subsumed) abstract arguments to the same callee, no redundant
//    re-analysis is triggered.
//
// `type Cursor = ()` — no cursor stack; `Core<V, ()>` matches the identity
// `Lift` / `Project` impls in lift.rs.
// ---------------------------------------------------------------------------

pub struct AbstractInterp<'ir, S: StageMeta, L: Dialect, V> {
    pipeline: &'ir Pipeline<S>,
    stage_id: CompileStage,
    widening: WideningStrategy,

    /// SSA values for the block currently being executed (shared slot,
    /// cleared at the start of every block).
    active_ssa: FxHashMap<SSAValue, V>,

    /// Per-function intraprocedural state (block entry maps, worklists).
    func_states: FxHashMap<SpecializedFunction, FuncState<V>>,

    /// Per-function summaries (joined input + abstract output).
    summaries: FxHashMap<SpecializedFunction, FuncSummary<V>>,

    /// Reverse call graph: callee → set of callers.
    /// Used to re-queue callers when a callee's output improves.
    callers: FxHashMap<SpecializedFunction, FxHashSet<SpecializedFunction>>,

    /// Number of times each function's summary input has been widened.
    /// Used to apply the interprocedural widening strategy to function inputs
    /// so that recursive calls converge (the intraprocedural block-level
    /// widening alone does not prevent unbounded input growth across
    /// function re-analyses).
    fn_visit_counts: FxHashMap<SpecializedFunction, usize>,

    /// Functions pending (re-)analysis.
    func_worklist: VecDeque<SpecializedFunction>,

    _phantom: PhantomData<L>,
}

// -- Env --------------------------------------------------------------------

impl<'ir, S, L, V> Env for AbstractInterp<'ir, S, L, V>
where
    S: StageMeta,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Value = V;
    type Effect = Core<V, ()>;
    type Error = InterpreterError;

    fn current_stage(&self) -> CompileStage {
        self.stage_id
    }

    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        self.active_ssa
            .get(&ssa)
            .cloned()
            .ok_or(InterpreterError::UnboundValue(ssa))
    }

    fn write(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        self.active_ssa.insert(SSAValue::from(r), v);
        Ok(())
    }

    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        self.active_ssa.insert(ssa, v);
        Ok(())
    }
}

// -- BaseDomain -------------------------------------------------------------

impl<'ir, S, L, V> BaseDomain for AbstractInterp<'ir, S, L, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Language = L;
    type Cursor = ();
    type StageContainer = S;

    fn stage_info_for<LD: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<LD>>
    where
        S: HasStageInfo<LD>,
    {
        self.pipeline.stage(stage_id)?.try_stage_info()
    }

    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError> {
        let stage_container = self
            .pipeline
            .stage(stage_id)
            .ok_or(InterpreterError::MissingEntry)?;
        let stage_info: &StageInfo<L> = stage_container
            .try_stage_info()
            .ok_or(InterpreterError::MissingEntry)?;
        let function = self
            .pipeline
            .resolve_function(stage_info, target)
            .ok_or(InterpreterError::MissingEntry)?;
        let staged_function = self
            .pipeline
            .function_info(function)
            .and_then(|info| info.staged_function(stage_id))
            .ok_or(InterpreterError::MissingEntry)?;
        staged_function
            .get_info(stage_info)
            .ok_or(InterpreterError::MissingEntry)?
            .unique_live_specialization()
            .map_err(|_| InterpreterError::UnhandledEffect("ambiguous specialization".into()))
    }
}

// -- Constructor ------------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone + AbstractValue> AbstractInterp<'ir, S, L, V> {
    pub fn new(pipeline: &'ir Pipeline<S>, stage_id: CompileStage) -> Self {
        Self::with_widening(pipeline, stage_id, WideningStrategy::Delayed(3))
    }

    pub fn with_widening(
        pipeline: &'ir Pipeline<S>,
        stage_id: CompileStage,
        widening: WideningStrategy,
    ) -> Self {
        Self {
            pipeline,
            stage_id,
            widening,
            active_ssa: FxHashMap::default(),
            func_states: FxHashMap::default(),
            summaries: FxHashMap::default(),
            callers: FxHashMap::default(),
            fn_visit_counts: FxHashMap::default(),
            func_worklist: VecDeque::new(),
            _phantom: PhantomData,
        }
    }
}

// -- Fixpoint analysis ------------------------------------------------------

impl<'ir, S, L, V> AbstractInterp<'ir, S, L, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect + Interpretable<Self, DialectEffect = Core<V, ()>>,
    V: Clone + AbstractValue,
{
    /// Run the interprocedural fixpoint from `entry_fn` with `args`.
    ///
    /// The analysis is demand-driven: only functions reachable from
    /// `entry_fn` (transitively via calls) are analysed.
    pub fn analyze(
        &mut self,
        entry_fn: SpecializedFunction,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        let entry_block = self.entry_block_of(entry_fn)?;
        self.summaries.insert(
            entry_fn,
            FuncSummary {
                input: args,
                output: None,
                entry_block,
            },
        );
        self.func_worklist.push_back(entry_fn);

        while let Some(func) = self.func_worklist.pop_front() {
            self.analyze_function(func)?;
        }

        Ok(self.summaries.get(&entry_fn).and_then(|s| s.output.clone()))
    }

    // -----------------------------------------------------------------------
    // Intraprocedural fixpoint for a single function
    // -----------------------------------------------------------------------

    fn analyze_function(&mut self, func: SpecializedFunction) -> Result<(), InterpreterError> {
        let (entry_block, input) = {
            let s = self
                .summaries
                .get(&func)
                .ok_or(InterpreterError::MissingEntry)?;
            (s.entry_block, s.input.clone())
        };

        // Fresh intraprocedural state: previous block states may be stale
        // if the function is being re-analyzed due to a changed input or an
        // improved callee output.
        let mut state = FuncState::new();
        state.block_in.insert(entry_block, input);
        state.block_worklist.push_back(entry_block);
        self.func_states.insert(func, state);

        // Clear the shared SSA slot: values from the previous function (or
        // previous run of this function) must not bleed into this analysis.
        // Within a single analysis pass we do NOT clear between blocks —
        // SSA values defined in dominating blocks remain visible to dominated
        // blocks, exactly as SSA scoping rules require.
        self.active_ssa.clear();

        loop {
            let block = {
                let state = self.func_states.get_mut(&func).unwrap();
                state.block_worklist.pop_front()
            };
            let Some(block) = block else { break };
            self.run_block(func, block)?;
        }

        Ok(())
    }

    fn run_block(
        &mut self,
        func: SpecializedFunction,
        block: Block,
    ) -> Result<(), InterpreterError> {
        // Phase 1: collect statement definitions. All borrows of `stage` end
        // before Phase 2 mutates `active_ssa`.
        let (ssa_keys, all_defs) = {
            let stage: &StageInfo<L> = self
                .pipeline
                .stage(self.stage_id)
                .and_then(|s| s.try_stage_info())
                .ok_or(InterpreterError::MissingEntry)?;

            let block_info = block.expect_info(stage);
            let ssa_keys: Vec<SSAValue> = block_info
                .arguments
                .iter()
                .map(|ba| SSAValue::from(*ba))
                .collect();

            let all_defs: Vec<L> = block
                .statements(stage)
                .chain(block.terminator(stage))
                .map(|s| s.definition(stage).clone())
                .collect();

            (ssa_keys, all_defs)
        };

        // Phase 2: bind block arguments from the current entry state.
        let entry_args = self
            .func_states
            .get(&func)
            .and_then(|s| s.block_in.get(&block).cloned())
            .unwrap_or_default();
        if ssa_keys.len() != entry_args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: ssa_keys.len(),
                got: entry_args.len(),
            });
        }
        // Bind block arguments, overwriting any stale value for these SSA
        // names from a prior pass. Do NOT clear the entire map here: values
        // defined in dominating blocks are still in scope in SSA and must
        // remain visible.
        for (ssa, val) in ssa_keys.into_iter().zip(entry_args) {
            self.active_ssa.insert(ssa, val);
        }

        // Phase 3: execute statements and dispatch Core effects.
        for def in all_defs {
            let effect = def.interpret(self)?;
            match effect {
                Core::Advance => {}
                Core::Jump(target, args) => {
                    self.propagate_in_fn(func, target, args)?;
                    return Ok(());
                }
                Core::Fork(b1, args1, b2, args2) => {
                    self.propagate_in_fn(func, b1, args1)?;
                    self.propagate_in_fn(func, b2, args2)?;
                    return Ok(());
                }
                Core::Return(v) | Core::Yield(v) => {
                    self.record_return(func, v)?;
                    return Ok(());
                }
                Core::Call {
                    callee,
                    args,
                    results,
                    ..
                } => {
                    // Non-recursive: look up or initialise the callee's
                    // summary. Returns the current output (bottom if the
                    // callee has not been analysed yet). The caller is
                    // re-queued when the callee's output improves.
                    let call_result = self.handle_call(func, callee, args)?;
                    for r in &results {
                        self.active_ssa
                            .insert(SSAValue::from(*r), call_result.clone());
                    }
                    // Continue executing — the call result is now bound.
                }
                Core::Push(_) | Core::Pop => {
                    return Err(InterpreterError::UnhandledEffect(
                        "Core::Push/Pop not supported in abstract interpreter; \
                         use structured control flow at an abstract-compatible stage"
                            .into(),
                    ));
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Intra-function propagation (block-level join + widening)
    // -----------------------------------------------------------------------

    fn propagate_in_fn(
        &mut self,
        func: SpecializedFunction,
        target: Block,
        args: Vec<V>,
    ) -> Result<(), InterpreterError> {
        let widening = self.widening;
        let state = self.func_states.get_mut(&func).unwrap();
        let visit_count = *state.visit_counts.get(&target).unwrap_or(&0);

        let changed = if let Some(existing) = state.block_in.get(&target) {
            if existing.len() != args.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: existing.len(),
                    got: args.len(),
                });
            }
            let new_args: Vec<V> = existing
                .iter()
                .zip(args.iter())
                .map(|(e, a)| widening.merge(e, a, visit_count))
                .collect();
            let changed = new_args
                .iter()
                .zip(existing.iter())
                .any(|(n, o)| !n.is_subseteq(o));
            if changed {
                state.block_in.insert(target, new_args);
            }
            changed
        } else {
            state.block_in.insert(target, args);
            true
        };

        if changed {
            *state.visit_counts.entry(target).or_insert(0) += 1;
            if !state.block_worklist.contains(&target) {
                state.block_worklist.push_back(target);
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Interprocedural summary update
    // -----------------------------------------------------------------------

    /// Process a call instruction without recursing into the Rust stack.
    ///
    /// If the callee is new or its abstract input grows, the callee is queued
    /// for (re-)analysis. The current (possibly bottom) output summary is
    /// returned as the call result so execution continues immediately.
    ///
    /// The function-level widening strategy is applied to the summary input
    /// so that recursive and mutually-recursive call chains converge: without
    /// it, each re-analysis would expand the input by one step and the
    /// outer fixpoint would never terminate.
    fn handle_call(
        &mut self,
        caller: SpecializedFunction,
        callee: SpecializedFunction,
        new_args: Vec<V>,
    ) -> Result<V, InterpreterError> {
        self.callers.entry(callee).or_default().insert(caller);

        if let Some(summary) = self.summaries.get(&callee) {
            let existing_input = summary.input.clone();

            if existing_input.len() != new_args.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: existing_input.len(),
                    got: new_args.len(),
                });
            }

            // Apply interprocedural widening based on how many times this
            // function's input has already been widened.
            let widening = self.widening;
            let fn_visits = *self.fn_visit_counts.get(&callee).unwrap_or(&0);
            let merged: Vec<V> = existing_input
                .iter()
                .zip(new_args.iter())
                .map(|(e, a)| widening.merge(e, a, fn_visits))
                .collect();
            let input_grew = merged
                .iter()
                .zip(existing_input.iter())
                .any(|(n, o)| !n.is_subseteq(o));

            if input_grew {
                self.summaries.get_mut(&callee).unwrap().input = merged;
                *self.fn_visit_counts.entry(callee).or_insert(0) += 1;
                // Queue for re-analysis. analyze_function always creates a
                // fresh FuncState at the start, so there is no need to remove
                // the existing state here — doing so would corrupt the live
                // analysis when callee == the currently-executing function
                // (i.e., a self-recursive call).
                if !self.func_worklist.contains(&callee) {
                    self.func_worklist.push_back(callee);
                }
            }

            // Return the current output, or bottom if not yet analysed.
            // The caller will be re-queued when the callee's output improves.
            Ok(self
                .summaries
                .get(&callee)
                .unwrap()
                .output
                .clone()
                .unwrap_or_else(V::bottom))
        } else {
            // First encounter: create summary with bottom output.
            let entry_block = self.entry_block_of(callee)?;
            self.summaries.insert(
                callee,
                FuncSummary {
                    input: new_args,
                    output: None,
                    entry_block,
                },
            );
            if !self.func_worklist.contains(&callee) {
                self.func_worklist.push_back(callee);
            }
            // Return bottom — callers are re-queued when output improves.
            Ok(V::bottom())
        }
    }

    /// Record a return value from `func`, joining it into the summary output.
    ///
    /// When the output grows, all known callers are invalidated and re-queued
    /// so that the improved precision propagates upward.
    fn record_return(&mut self, func: SpecializedFunction, v: V) -> Result<(), InterpreterError> {
        let summary = self
            .summaries
            .get_mut(&func)
            .ok_or(InterpreterError::MissingEntry)?;
        let new_output = match &summary.output {
            None => v,
            Some(existing) => existing.join(&v),
        };
        let output_grew = match &summary.output {
            None => true,
            Some(existing) => !new_output.is_subseteq(existing),
        };
        summary.output = Some(new_output);

        if output_grew {
            // Re-queue all callers so they re-propagate the improved result.
            // analyze_function always creates a fresh FuncState, so we do not
            // remove states here — that would corrupt any caller that is
            // currently executing (e.g., mutual recursion).
            if let Some(callers) = self.callers.get(&func).cloned() {
                for caller in callers {
                    if !self.func_worklist.contains(&caller) {
                        self.func_worklist.push_back(caller);
                    }
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helper
    // -----------------------------------------------------------------------

    fn entry_block_of(&self, func: SpecializedFunction) -> Result<Block, InterpreterError> {
        let stage: &StageInfo<L> = self
            .pipeline
            .stage(self.stage_id)
            .and_then(|s| s.try_stage_info())
            .ok_or(InterpreterError::MissingEntry)?;
        let spec_info = func.get_info(stage).ok_or(InterpreterError::MissingEntry)?;
        let body_stmt = *spec_info.body();
        let definition = body_stmt.definition(stage).clone();
        definition
            .regions()
            .next()
            .ok_or(InterpreterError::MissingEntry)
            .and_then(|region| {
                region
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::MissingEntry)
            })
    }
}
