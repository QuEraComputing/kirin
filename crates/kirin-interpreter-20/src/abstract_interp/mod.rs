pub mod state;

pub use self::state::{AbstractFrame, AnalysisResult, FuncState, FuncSummary, StagedKey, Worklist};

use std::marker::PhantomData;

use kirin_interpreter::{AbstractValue, ProductValue, WideningStrategy};
use kirin_ir::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageMeta,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::abstract_call_dispatch::AbstractCallDispatch;
use crate::block_exec::{BlockExecEnv, JumpOutcome};
use crate::control::{Control, CursorExt};
use crate::env::{AbstractEnv, AbstractMode, Env};
use crate::error::InterpreterError;
use crate::execute::{Execute, StackEntry};
use crate::fixpoint_driver::FixpointDriver;
use crate::pipeline::PipelineHandle;

// ---------------------------------------------------------------------------
// AbstractInterp
// ---------------------------------------------------------------------------

pub struct AbstractInterp<'ir, S: StageMeta, L: Dialect, V, C> {
    pub handle: PipelineHandle<'ir, S>,
    pub widening: WideningStrategy,
    pub func_states: FxHashMap<StagedKey, FuncState<V>>,
    pub summaries: FxHashMap<StagedKey, FuncSummary<V>>,
    pub call_graph: FxHashMap<StagedKey, FxHashSet<state::AbstractFrame>>,
    pub fn_visit_counts: FxHashMap<StagedKey, usize>,
    pub func_worklist: state::Worklist<StagedKey>,
    pub current_key: Option<StagedKey>,
    pub cursor_stack: Vec<StackEntry<C, V>>,
    pub narrowing_iterations: usize,
    pub(crate) narrowing_mode: bool,
    pub _phantom: PhantomData<L>,
}

// -- Env impl ----------------------------------------------------------------

impl<'ir, S, L, V, C> Env for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Mode = AbstractMode<C>;
    type Value = V;
    type Ext = CursorExt<C>;
    type Error = InterpreterError;
    type Stages = S;

    fn current_stage(&self) -> CompileStage {
        self.current_key
            .map(|(_, s)| s)
            .unwrap_or(self.handle.stage_id)
    }

    fn pipeline(&self) -> &Pipeline<S> {
        self.handle.pipeline
    }

    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        let key = self.current_key.ok_or(InterpreterError::NoFrame)?;
        Ok(self
            .func_states
            .get(&key)
            .and_then(|s| s.active_ssa.get(&ssa))
            .cloned()
            .unwrap_or_else(V::bottom))
    }

    fn write_result(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        let key = self.current_key.ok_or(InterpreterError::NoFrame)?;
        self.func_states
            .get_mut(&key)
            .ok_or(InterpreterError::NoFrame)?
            .active_ssa
            .insert(SSAValue::from(r), v);
        Ok(())
    }

    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        let key = self.current_key.ok_or(InterpreterError::NoFrame)?;
        self.func_states
            .get_mut(&key)
            .ok_or(InterpreterError::NoFrame)?
            .active_ssa
            .insert(ssa, v);
        Ok(())
    }
}

// -- AbstractEnv impl --------------------------------------------------------

impl<'ir, S, L, V, C> AbstractEnv for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    fn enqueue_block(&mut self, block: Block, args: Vec<V>) {
        let Some(key) = self.current_key else { return };
        let Some(state) = self.func_states.get_mut(&key) else {
            return;
        };

        let narrowing_mode = self.narrowing_mode;
        let changed = if let Some(existing) = state.block_in.get(&block) {
            if existing.len() != args.len() {
                state.block_in.insert(block, args);
                true
            } else {
                let new_args: Vec<V> = if narrowing_mode {
                    existing
                        .iter()
                        .zip(args.iter())
                        .map(|(e, a)| e.narrow(a))
                        .collect()
                } else {
                    let widening = self.widening;
                    let visit_count = *state.visit_counts.get(&block).unwrap_or(&0);
                    existing
                        .iter()
                        .zip(args.iter())
                        .map(|(e, a)| widening.merge(e, a, visit_count))
                        .collect()
                };
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
            state.block_worklist.push(block);
        }
    }

    fn record_return(&mut self, v: V) -> Result<(), InterpreterError> {
        let key = self.current_key.ok_or(InterpreterError::NoFrame)?;
        self.record_return_inner(key, v)
    }

    fn current_function(&self) -> SpecializedFunction {
        self.current_key
            .expect("AbstractEnv::current_function called outside of analyze_function")
            .0
    }
}

// -- BlockExecEnv impl — abstract mode: jumps enqueue blocks ----------------

impl<'ir, S, L, V, C> BlockExecEnv for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    fn exec_jump(&mut self, target: Block, args: Vec<V>) -> JumpOutcome<V, CursorExt<C>> {
        self.enqueue_block(target, args);
        JumpOutcome::Done(Control::Ext(CursorExt::Pop))
    }

    fn exec_fork(
        &mut self,
        branches: Vec<(Block, Vec<V>)>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        for (block, args) in branches {
            self.enqueue_block(block, args);
        }
        Ok(Control::Ext(CursorExt::Pop))
    }

    fn exec_block_end(&self) -> Control<V, CursorExt<C>> {
        Control::Ext(CursorExt::Pop)
    }
}

// -- FixpointDriver impl -----------------------------------------------------

impl<'ir, S, L, V, C> FixpointDriver for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Cursor = C;

    fn summaries_ref(&self) -> &FxHashMap<StagedKey, FuncSummary<V>> {
        &self.summaries
    }
    fn summaries_mut(&mut self) -> &mut FxHashMap<StagedKey, FuncSummary<V>> {
        &mut self.summaries
    }
    fn func_states_mut(&mut self) -> &mut FxHashMap<StagedKey, FuncState<V>> {
        &mut self.func_states
    }
    fn func_worklist_mut(&mut self) -> &mut Worklist<StagedKey> {
        &mut self.func_worklist
    }
    fn cursor_stack_ref(&self) -> &[StackEntry<C, V>] {
        &self.cursor_stack
    }
    fn cursor_stack_mut(&mut self) -> &mut Vec<StackEntry<C, V>> {
        &mut self.cursor_stack
    }
    fn call_graph_mut(&mut self) -> &mut FxHashMap<StagedKey, FxHashSet<AbstractFrame>> {
        &mut self.call_graph
    }
    fn fn_visit_counts_mut(&mut self) -> &mut FxHashMap<StagedKey, usize> {
        &mut self.fn_visit_counts
    }
    fn widening_strategy(&self) -> WideningStrategy {
        self.widening
    }
    fn make_abstract_cursor(&self, stage_id: CompileStage, block: Block, args: Vec<V>) -> C {
        S::make_abstract_cursor(self.handle.pipeline, stage_id, block, args)
    }
    fn set_current_key(&mut self, key: Option<StagedKey>) {
        self.current_key = key;
    }
    fn get_current_key(&self) -> Option<StagedKey> {
        self.current_key
    }
    fn entry_block_for(
        &self,
        func: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError> {
        S::entry_block_for(self.handle.pipeline, func, stage_id)
    }
}

// -- Internal helpers --------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone + AbstractValue, C> AbstractInterp<'ir, S, L, V, C> {
    pub fn record_return_inner(&mut self, key: StagedKey, v: V) -> Result<(), InterpreterError> {
        let summary = self
            .summaries
            .get_mut(&key)
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
            let caller_keys: Vec<StagedKey> = self
                .call_graph
                .get(&key)
                .into_iter()
                .flatten()
                .map(|f| (f.func, f.stage))
                .collect();
            for ck in caller_keys {
                self.func_worklist.push(ck);
            }
        }
        Ok(())
    }
}

// -- Constructor -------------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone + AbstractValue, C> AbstractInterp<'ir, S, L, V, C> {
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
            call_graph: FxHashMap::default(),
            fn_visit_counts: FxHashMap::default(),
            func_worklist: state::Worklist::new(),
            current_key: None,
            cursor_stack: Vec::new(),
            narrowing_iterations: 0,
            narrowing_mode: false,
            _phantom: PhantomData,
        }
    }

    pub fn with_narrowing_iterations(mut self, n: usize) -> Self {
        self.narrowing_iterations = n;
        self
    }
}

// -- Fixpoint analysis (delegated to FixpointDriver) -------------------------

impl<'ir, S, L, V, C> AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    C: Execute<Self>,
    Self: Env<Ext = CursorExt<C>, Value = V, Error = InterpreterError>,
{
    pub fn analyze(
        &mut self,
        entry_fn: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        FixpointDriver::run_fixpoint(self, entry_fn, stage_id, args)?;
        if self.narrowing_iterations > 0 {
            self.run_narrowing()?;
        }
        Ok(self
            .summaries
            .get(&(entry_fn, stage_id))
            .and_then(|s| s.output.clone()))
    }

    fn run_narrowing(&mut self) -> Result<(), InterpreterError> {
        self.narrowing_mode = true;
        let keys: Vec<StagedKey> = self.summaries.keys().cloned().collect();
        for _ in 0..self.narrowing_iterations {
            for &key in &keys {
                crate::fixpoint_driver::analyze_function(self, key)?;
            }
        }
        self.narrowing_mode = false;
        Ok(())
    }

    pub fn analysis_result(
        &self,
        func: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Option<state::AnalysisResult<V>> {
        let key = (func, stage_id);
        let func_state = self.func_states.get(&key)?;
        let return_value = self.summaries.get(&key).and_then(|s| s.output.clone());
        Some(state::AnalysisResult {
            block_in: func_state.block_in.clone(),
            ssa_values: func_state.active_ssa.clone(),
            return_value,
        })
    }
}
