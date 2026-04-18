use std::collections::VecDeque;
use std::convert::Infallible;
use std::marker::PhantomData;

use kirin_interpreter::{AbstractValue, WideningStrategy};
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta, Symbol,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::control::Control;
use crate::env::{AbstractEnv, Interpretable};
use crate::error::InterpreterError;
use crate::interp::Interp;
use crate::pipeline::PipelineHandle;
use crate::store::Store;

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
    /// SSA values for the block currently being executed.
    ///
    /// Moved from `AbstractInterp` (fixes P6 from interpreter-6): keeping
    /// `active_ssa` per-function allows clean re-entrant analysis and avoids
    /// the shared-mutable-field footgun.
    active_ssa: FxHashMap<SSAValue, V>,
}

impl<V> FuncState<V> {
    fn new() -> Self {
        Self {
            block_in: FxHashMap::default(),
            visit_counts: FxHashMap::default(),
            block_worklist: VecDeque::new(),
            active_ssa: FxHashMap::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Interprocedural summary
// ---------------------------------------------------------------------------

struct FuncSummary<V> {
    /// Joined abstract inputs seen across all call sites.
    input: Vec<V>,
    /// Abstract return value (None = bottom, not yet analyzed).
    output: Option<V>,
    /// Cached entry block.
    entry_block: Block,
}

// ---------------------------------------------------------------------------
// AbstractInterp
// ---------------------------------------------------------------------------

/// Summary-based interprocedural fixpoint interpreter.
///
/// Correctness properties:
///
/// 1. **No Rust-stack recursion.** Calls look up/initialise the callee summary
///    and return the current output. The callee is queued for re-analysis; the
///    outer loop drives everything.
///
/// 2. **Cycle detection.** Every function has a summary keyed on
///    `SpecializedFunction`; re-entrant calls return the current (possibly bottom)
///    output instead of looping.
///
/// 3. **`active_ssa` is per-`FuncState`** (fixes interpreter-6 P6): values from
///    a previous function no longer bleed into the current one.
pub struct AbstractInterp<'ir, S: StageMeta, L: Dialect, V> {
    handle: PipelineHandle<'ir, S>,
    widening: WideningStrategy,

    /// Per-function intraprocedural state.
    func_states: FxHashMap<SpecializedFunction, FuncState<V>>,

    /// Per-function summaries (joined input + abstract output).
    summaries: FxHashMap<SpecializedFunction, FuncSummary<V>>,

    /// Reverse call graph: callee → set of callers.
    callers: FxHashMap<SpecializedFunction, FxHashSet<SpecializedFunction>>,

    /// Number of times each function's summary input has been widened.
    fn_visit_counts: FxHashMap<SpecializedFunction, usize>,

    /// Functions pending (re-)analysis.
    func_worklist: VecDeque<SpecializedFunction>,

    /// The function currently being analyzed. Set while `analyze_function` runs;
    /// used by `AbstractEnv` method impls (e.g. `enqueue_block`).
    current_func: Option<SpecializedFunction>,

    _phantom: PhantomData<L>,
}

// -- Store ------------------------------------------------------------------

impl<'ir, S, L, V> Store for AbstractInterp<'ir, S, L, V>
where
    S: StageMeta,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Value = V;
    type Error = InterpreterError;

    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        let func = self.current_func.ok_or(InterpreterError::NoFrame)?;
        self.func_states
            .get(&func)
            .and_then(|s| s.active_ssa.get(&ssa))
            .cloned()
            .ok_or(InterpreterError::UnboundValue(ssa))
    }

    fn write_result(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        let func = self.current_func.ok_or(InterpreterError::NoFrame)?;
        self.func_states
            .get_mut(&func)
            .ok_or(InterpreterError::NoFrame)?
            .active_ssa
            .insert(SSAValue::from(r), v);
        Ok(())
    }

    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        let func = self.current_func.ok_or(InterpreterError::NoFrame)?;
        self.func_states
            .get_mut(&func)
            .ok_or(InterpreterError::NoFrame)?
            .active_ssa
            .insert(ssa, v);
        Ok(())
    }
}

// -- Interp -----------------------------------------------------------------

impl<'ir, S, L, V> Interp for AbstractInterp<'ir, S, L, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Dialect = L;
    /// Abstract mode: no cursor events occur.
    type Ext = Infallible;
    type StageContainer = S;

    fn current_stage(&self) -> CompileStage {
        self.handle.stage_id
    }

    fn stage_info_for<LD: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<LD>>
    where
        S: HasStageInfo<LD>,
    {
        self.handle.stage_info_for::<LD>(stage_id)
    }

    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError>
    where
        S: HasStageInfo<L>,
    {
        self.handle.resolve_function_for::<L>(target, stage_id)
    }
}

// -- AbstractEnv ------------------------------------------------------------

impl<'ir, S, L, V> AbstractEnv for AbstractInterp<'ir, S, L, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    fn enqueue_block(&mut self, block: Block, args: Vec<V>) {
        if let Some(func) = self.current_func {
            // Use propagate_in_fn logic: join args and enqueue if changed.
            // For simplicity here we just insert directly — the caller
            // (SCF abstract impl) is responsible for reasonable args.
            if let Some(state) = self.func_states.get_mut(&func) {
                let changed = if let Some(existing) = state.block_in.get(&block) {
                    if existing.len() != args.len() {
                        // Arity mismatch — store as-is to avoid panic.
                        state.block_in.insert(block, args);
                        true
                    } else {
                        let widening = self.widening;
                        let visit_count = *state.visit_counts.get(&block).unwrap_or(&0);
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
                            state.block_in.insert(block, new_args);
                        }
                        changed
                    }
                } else {
                    state.block_in.insert(block, args);
                    true
                };

                if changed {
                    *state.visit_counts.entry(block).or_insert(0) += 1;
                    if !state.block_worklist.contains(&block) {
                        state.block_worklist.push_back(block);
                    }
                }
            }
        }
    }

    fn record_return(&mut self, v: V) -> Result<(), InterpreterError> {
        let func = self.current_func.ok_or(InterpreterError::NoFrame)?;
        // record_return_inner is defined on the base struct impl (no dialect bounds)
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

        if output_grew && let Some(callers) = self.callers.get(&func).cloned() {
            for caller in callers {
                if !self.func_worklist.contains(&caller) {
                    self.func_worklist.push_back(caller);
                }
            }
        }

        Ok(())
    }

    fn current_function(&self) -> SpecializedFunction {
        self.current_func
            .expect("AbstractEnv::current_function called outside of analyze_function")
    }
}

// -- Internal helpers (no dialect constraints) ------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone + AbstractValue> AbstractInterp<'ir, S, L, V> {
    fn record_return_inner(
        &mut self,
        func: SpecializedFunction,
        v: V,
    ) -> Result<(), InterpreterError> {
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

        if output_grew && let Some(callers) = self.callers.get(&func).cloned() {
            for caller in callers {
                if !self.func_worklist.contains(&caller) {
                    self.func_worklist.push_back(caller);
                }
            }
        }

        Ok(())
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
            handle: PipelineHandle::new(pipeline, stage_id),
            widening,
            func_states: FxHashMap::default(),
            summaries: FxHashMap::default(),
            callers: FxHashMap::default(),
            fn_visit_counts: FxHashMap::default(),
            func_worklist: VecDeque::new(),
            current_func: None,
            _phantom: PhantomData,
        }
    }
}

// -- Fixpoint analysis ------------------------------------------------------

impl<'ir, S, L, V> AbstractInterp<'ir, S, L, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect + Interpretable<Self, Effect = Control<V, Infallible>>,
    V: Clone + AbstractValue,
{
    /// Run the interprocedural fixpoint from `entry_fn` with `args`.
    pub fn analyze(
        &mut self,
        entry_fn: SpecializedFunction,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        let entry_block = self
            .handle
            .entry_block_of::<L>(entry_fn, self.handle.stage_id)?;
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

    fn analyze_function(&mut self, func: SpecializedFunction) -> Result<(), InterpreterError> {
        let (entry_block, input) = {
            let s = self
                .summaries
                .get(&func)
                .ok_or(InterpreterError::MissingEntry)?;
            (s.entry_block, s.input.clone())
        };

        let mut state = FuncState::new();
        state.block_in.insert(entry_block, input);
        state.block_worklist.push_back(entry_block);
        self.func_states.insert(func, state);

        self.current_func = Some(func);

        loop {
            let block = {
                let state = self.func_states.get_mut(&func).unwrap();
                state.block_worklist.pop_front()
            };
            let Some(block) = block else { break };
            self.run_block(func, block)?;
        }

        self.current_func = None;
        Ok(())
    }

    fn run_block(
        &mut self,
        func: SpecializedFunction,
        block: Block,
    ) -> Result<(), InterpreterError> {
        // Phase 1: collect statement definitions (releases all borrows).
        let (ssa_keys, all_defs) = {
            let stage: &StageInfo<L> = self
                .handle
                .pipeline
                .stage(self.handle.stage_id)
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

        // Phase 2: bind block arguments.
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
        for (ssa, val) in ssa_keys.into_iter().zip(entry_args) {
            if let Some(state) = self.func_states.get_mut(&func) {
                state.active_ssa.insert(ssa, val);
            }
        }

        // Phase 3: execute statements and dispatch Control effects.
        for def in all_defs {
            let effect: Control<V, Infallible> = def.interpret(self)?;
            match effect {
                Control::Advance => {}
                Control::Jump(target, args) => {
                    self.propagate_in_fn(func, target, args)?;
                    return Ok(());
                }
                Control::Fork(b1, args1, b2, args2) => {
                    self.propagate_in_fn(func, b1, args1)?;
                    self.propagate_in_fn(func, b2, args2)?;
                    return Ok(());
                }
                Control::Return(v) | Control::Yield(v) => {
                    self.record_return_inner(func, v)?;
                    return Ok(());
                }
                Control::Call {
                    callee,
                    args,
                    results,
                    ..
                } => {
                    let call_result = self.handle_call(func, callee, args)?;
                    for r in &results {
                        if let Some(state) = self.func_states.get_mut(&func) {
                            state
                                .active_ssa
                                .insert(SSAValue::from(*r), call_result.clone());
                        }
                    }
                }
                Control::Ext(e) => match e {}, // Infallible — unreachable
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Intra-function propagation
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
    // Interprocedural summary
    // -----------------------------------------------------------------------

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
                if !self.func_worklist.contains(&callee) {
                    self.func_worklist.push_back(callee);
                }
            }

            Ok(self
                .summaries
                .get(&callee)
                .unwrap()
                .output
                .clone()
                .unwrap_or_else(V::bottom))
        } else {
            let entry_block = self
                .handle
                .entry_block_of::<L>(callee, self.handle.stage_id)?;
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
            Ok(V::bottom())
        }
    }
}
