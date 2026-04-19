use std::collections::VecDeque;
use std::marker::PhantomData;

use kirin_interpreter::{AbstractValue, WideningStrategy};
use kirin_ir::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageMeta,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::algebra::Lift;
use crate::control::{Control, CursorExt};
use crate::cursor::AbstractBlockCursor;
use crate::env::{AbstractEnv, AbstractMode, Env};
use crate::error::InterpreterError;
use crate::execute::{Execute, StackEntry};
use crate::pipeline::PipelineHandle;

// ---------------------------------------------------------------------------
// Intraprocedural state
// ---------------------------------------------------------------------------

struct FuncState<V> {
    block_in: FxHashMap<Block, Vec<V>>,
    visit_counts: FxHashMap<Block, usize>,
    block_worklist: VecDeque<Block>,
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
    input: Vec<V>,
    output: Option<V>,
    entry_block: Block,
}

// ---------------------------------------------------------------------------
// AbstractInterp
// ---------------------------------------------------------------------------

/// Summary-based interprocedural fixpoint interpreter.
///
/// `C` is the abstract cursor coproduct. For flat-CF programs, use
/// `AbstractBlockCursor<V, L>` directly (the identity `Lift` impl applies).
/// For SCF programs, use a coproduct that includes `AbstractBlockCursor<V, L>`
/// and abstract SCF cursors (e.g. `AbstractIfCursor`, `AbstractForCursor`).
///
/// # Correctness fix vs interpreter-9
/// Abstract SCF cursors must use `AbstractBlockCursor` for their body execution,
/// not `BlockCursor`. `AbstractBlockCursor: Execute<AbstractInterp>` is the
/// correct impl; `BlockCursor: Execute<AbstractInterp>` does not type-check
/// because `BlockCursor` requires `E: Env<Mode = ConcreteMode<C>>`.
pub struct AbstractInterp<'ir, S: StageMeta, L: Dialect, V, C> {
    handle: PipelineHandle<'ir, S>,
    widening: WideningStrategy,
    func_states: FxHashMap<SpecializedFunction, FuncState<V>>,
    summaries: FxHashMap<SpecializedFunction, FuncSummary<V>>,
    callers: FxHashMap<SpecializedFunction, FxHashSet<SpecializedFunction>>,
    fn_visit_counts: FxHashMap<SpecializedFunction, usize>,
    func_worklist: VecDeque<SpecializedFunction>,
    current_func: Option<SpecializedFunction>,
    cursor_stack: Vec<StackEntry<C, V>>,
    _phantom: PhantomData<L>,
}

// -- Env -----------------------------------------------------------------

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
        self.handle.stage_id
    }

    fn pipeline(&self) -> &kirin_ir::Pipeline<S> {
        self.handle.pipeline
    }

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

// -- AbstractEnv ---------------------------------------------------------

impl<'ir, S, L, V, C> AbstractEnv for AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    fn enqueue_block(&mut self, block: Block, args: Vec<V>) {
        if let Some(func) = self.current_func
            && let Some(state) = self.func_states.get_mut(&func)
        {
            let changed = if let Some(existing) = state.block_in.get(&block) {
                if existing.len() != args.len() {
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

    fn record_return(&mut self, v: V) -> Result<(), InterpreterError> {
        let func = self.current_func.ok_or(InterpreterError::NoFrame)?;
        self.record_return_inner(func, v)
    }

    fn current_function(&self) -> SpecializedFunction {
        self.current_func
            .expect("AbstractEnv::current_function called outside of analyze_function")
    }
}

// -- Internal helpers -------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone + AbstractValue, C> AbstractInterp<'ir, S, L, V, C> {
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
            callers: FxHashMap::default(),
            fn_visit_counts: FxHashMap::default(),
            func_worklist: VecDeque::new(),
            current_func: None,
            cursor_stack: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

// -- Fixpoint analysis ------------------------------------------------------

impl<'ir, S, L, V, C> AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
    C: Execute<Self>,
    AbstractBlockCursor<V, L>: Execute<Self> + Lift<C>,
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
        state.block_in.insert(entry_block, input.clone());
        state.block_worklist.push_back(entry_block);
        self.func_states.insert(func, state);
        self.current_func = Some(func);

        loop {
            while !self.cursor_stack.is_empty() {
                self.step_cursor(func)?;
            }

            let block = {
                let state = self.func_states.get_mut(&func).unwrap();
                state.block_worklist.pop_front()
            };
            let Some(block) = block else { break };

            let block_args = self
                .func_states
                .get(&func)
                .and_then(|s| s.block_in.get(&block).cloned())
                .unwrap_or_default();

            let cursor = AbstractBlockCursor::<V, L>::new(block, self.handle.stage_id, block_args);
            self.cursor_stack.push(StackEntry::new(cursor.lift()));

            while !self.cursor_stack.is_empty() {
                self.step_cursor(func)?;
            }
        }

        self.current_func = None;
        Ok(())
    }

    fn step_cursor(&mut self, func: SpecializedFunction) -> Result<(), InterpreterError> {
        let Some(mut entry) = self.cursor_stack.pop() else {
            return Ok(());
        };

        let inbox = entry.inbox.take();
        let effect: Control<V, CursorExt<C>> = entry.cursor.execute(self, inbox)?;

        match effect {
            Control::Advance => {
                self.cursor_stack.push(entry);
            }
            Control::Ext(CursorExt::Push(new_cursor)) => {
                self.cursor_stack.push(entry);
                self.cursor_stack.push(StackEntry::new(new_cursor));
            }
            Control::Ext(CursorExt::Pop) => {
                // Cursor done; discard.
            }
            Control::Yield(v) => {
                if let Some(parent) = self.cursor_stack.last_mut() {
                    parent.inbox = Some(v);
                }
            }
            Control::Return(v) => {
                self.cursor_stack.clear();
                self.record_return_inner(func, v)?;
            }
            Control::Jump(block, args) => {
                self.enqueue_block(block, args);
            }
            Control::Fork(branches) => {
                for (block, args) in branches {
                    self.enqueue_block(block, args);
                }
            }
            Control::Call {
                callee,
                args,
                results,
                ..
            } => {
                self.cursor_stack.push(entry);
                let call_result = self.handle_call(func, callee, args)?;
                for r in &results {
                    self.func_states
                        .get_mut(&func)
                        .ok_or(InterpreterError::NoFrame)?
                        .active_ssa
                        .insert(SSAValue::from(*r), call_result.clone());
                }
            }
        }

        Ok(())
    }

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
