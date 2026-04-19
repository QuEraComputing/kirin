use std::collections::VecDeque;
use std::hash::Hash;
use std::marker::PhantomData;

use kirin_interpreter::{AbstractValue, WideningStrategy};
use kirin_ir::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageMeta,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::abstract_call_dispatch::AbstractCallDispatch;
use crate::context::AbstractFrame;
use crate::control::{Control, CursorExt};
use crate::env::{AbstractEnv, AbstractMode, Env};
use crate::error::InterpreterError;
use crate::execute::{Execute, StackEntry};
use crate::pipeline::PipelineHandle;

// ---------------------------------------------------------------------------
// O(1) deduplicating worklist
// ---------------------------------------------------------------------------

/// A worklist with O(1) membership tests for dedup.
///
/// Enqueuing an item already in the queue is a no-op. Dequeue preserves FIFO order.
struct Worklist<T: Hash + Eq> {
    queue: VecDeque<T>,
    set: FxHashSet<T>,
}

impl<T: Hash + Eq + Clone> Worklist<T> {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            set: FxHashSet::default(),
        }
    }

    /// Enqueue `item` if not already present. Returns true if added.
    fn push(&mut self, item: T) -> bool {
        if self.set.contains(&item) {
            return false;
        }
        self.set.insert(item.clone());
        self.queue.push_back(item);
        true
    }

    /// Dequeue the next item, removing it from the membership set.
    fn pop(&mut self) -> Option<T> {
        let item = self.queue.pop_front()?;
        self.set.remove(&item);
        Some(item)
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Function key — (function, stage) pair, Copy
// ---------------------------------------------------------------------------

type StagedKey = (SpecializedFunction, CompileStage);

// ---------------------------------------------------------------------------
// Intraprocedural state
// ---------------------------------------------------------------------------

struct FuncState<V> {
    block_in: FxHashMap<Block, Vec<V>>,
    visit_counts: FxHashMap<Block, usize>,
    block_worklist: Worklist<Block>,
    active_ssa: FxHashMap<SSAValue, V>,
}

impl<V> FuncState<V> {
    fn new() -> Self {
        Self {
            block_in: FxHashMap::default(),
            visit_counts: FxHashMap::default(),
            block_worklist: Worklist::new(),
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
/// Each function is identified by `(SpecializedFunction, CompileStage)` and
/// has exactly one summary.  The abstract call graph records which call sites
/// reach each function, enabling callers to be re-enqueued when a callee's
/// summary output grows.
///
/// `C` is the abstract cursor coproduct. For flat-CF programs use
/// `AbstractBlockCursor<V, L>` directly.  For SCF programs use a coproduct
/// that includes `AbstractBlockCursor<V, L>` and the abstract SCF cursors.
pub struct AbstractInterp<'ir, S: StageMeta, L: Dialect, V, C> {
    handle: PipelineHandle<'ir, S>,
    widening: WideningStrategy,
    func_states: FxHashMap<StagedKey, FuncState<V>>,
    summaries: FxHashMap<StagedKey, FuncSummary<V>>,
    /// Abstract call graph: callee → set of call sites (frames) that invoke it.
    call_graph: FxHashMap<StagedKey, FxHashSet<AbstractFrame>>,
    fn_visit_counts: FxHashMap<StagedKey, usize>,
    func_worklist: Worklist<StagedKey>,
    current_key: Option<StagedKey>,
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
        self.current_key
            .map(|(_, s)| s)
            .unwrap_or(self.handle.stage_id)
    }

    fn pipeline(&self) -> &kirin_ir::Pipeline<S> {
        self.handle.pipeline
    }

    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        let key = self.current_key.ok_or(InterpreterError::NoFrame)?;
        self.func_states
            .get(&key)
            .and_then(|s| s.active_ssa.get(&ssa))
            .cloned()
            .ok_or(InterpreterError::UnboundValue(ssa))
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

// -- AbstractEnv ---------------------------------------------------------

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

// -- Internal helpers -------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone + AbstractValue, C> AbstractInterp<'ir, S, L, V, C> {
    fn record_return_inner(&mut self, key: StagedKey, v: V) -> Result<(), InterpreterError> {
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
            // Re-enqueue all callers recorded in the abstract call graph.
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
            call_graph: FxHashMap::default(),
            fn_visit_counts: FxHashMap::default(),
            func_worklist: Worklist::new(),
            current_key: None,
            cursor_stack: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

// -- Fixpoint analysis ------------------------------------------------------

impl<'ir, S, L, V, C> AbstractInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue,
    C: Execute<Self>,
{
    /// Run the interprocedural fixpoint from `entry_fn` at `stage_id` with `args`.
    pub fn analyze(
        &mut self,
        entry_fn: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        let entry_block = S::entry_block_for(self.handle.pipeline, entry_fn, stage_id)?;
        let entry_key = (entry_fn, stage_id);
        self.summaries.insert(
            entry_key,
            FuncSummary {
                input: args,
                output: None,
                entry_block,
            },
        );
        self.func_worklist.push(entry_key);

        while let Some(key) = self.func_worklist.pop() {
            self.analyze_function(key)?;
        }

        Ok(self
            .summaries
            .get(&entry_key)
            .and_then(|s| s.output.clone()))
    }

    fn analyze_function(&mut self, key: StagedKey) -> Result<(), InterpreterError> {
        let (_, func_stage) = key;
        let (entry_block, input) = {
            let s = self
                .summaries
                .get(&key)
                .ok_or(InterpreterError::MissingEntry)?;
            (s.entry_block, s.input.clone())
        };

        let mut state = FuncState::new();
        state.block_in.insert(entry_block, input);
        state.block_worklist.push(entry_block);
        self.func_states.insert(key, state);
        self.current_key = Some(key);

        loop {
            while !self.cursor_stack.is_empty() {
                self.step_cursor(key)?;
            }

            let block = {
                let state = self.func_states.get_mut(&key).unwrap();
                state.block_worklist.pop()
            };
            let Some(block) = block else { break };

            let block_args = self
                .func_states
                .get(&key)
                .and_then(|s| s.block_in.get(&block).cloned())
                .unwrap_or_default();

            let cursor =
                S::make_abstract_cursor(self.handle.pipeline, func_stage, block, block_args);
            self.cursor_stack.push(StackEntry::new(cursor));

            while !self.cursor_stack.is_empty() {
                self.step_cursor(key)?;
            }
        }

        self.current_key = None;
        Ok(())
    }

    fn step_cursor(&mut self, key: StagedKey) -> Result<(), InterpreterError> {
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
            Control::Ext(CursorExt::Pop) => {}
            Control::Yield(v) => {
                if let Some(parent) = self.cursor_stack.last_mut() {
                    parent.inbox = Some(v);
                }
            }
            Control::Return(v) => {
                self.cursor_stack.clear();
                self.record_return_inner(key, v)?;
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
                stage: callee_stage,
                args,
                results,
            } => {
                self.cursor_stack.push(entry);
                let call_result = self.handle_call(key, callee, callee_stage, &results, args)?;
                for r in &results {
                    self.func_states
                        .get_mut(&key)
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
        caller_key: StagedKey,
        callee: SpecializedFunction,
        callee_stage: CompileStage,
        call_site_results: &[ResultValue],
        new_args: Vec<V>,
    ) -> Result<V, InterpreterError> {
        let callee_key = (callee, callee_stage);

        // Record this call site in the abstract call graph.
        let frame = AbstractFrame {
            func: caller_key.0,
            stage: caller_key.1,
            results: call_site_results.to_vec(),
        };
        self.call_graph.entry(callee_key).or_default().insert(frame);

        if let Some(summary) = self.summaries.get(&callee_key) {
            let existing_input = summary.input.clone();

            if existing_input.len() != new_args.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: existing_input.len(),
                    got: new_args.len(),
                });
            }

            let widening = self.widening;
            let fn_visits = *self.fn_visit_counts.get(&callee_key).unwrap_or(&0);
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
                self.summaries.get_mut(&callee_key).unwrap().input = merged;
                *self.fn_visit_counts.entry(callee_key).or_insert(0) += 1;
                self.func_worklist.push(callee_key);
            }

            Ok(self
                .summaries
                .get(&callee_key)
                .unwrap()
                .output
                .clone()
                .unwrap_or_else(V::bottom))
        } else {
            let entry_block = S::entry_block_for(self.handle.pipeline, callee, callee_stage)?;
            self.summaries.insert(
                callee_key,
                FuncSummary {
                    input: new_args,
                    output: None,
                    entry_block,
                },
            );
            self.func_worklist.push(callee_key);
            Ok(V::bottom())
        }
    }
}
