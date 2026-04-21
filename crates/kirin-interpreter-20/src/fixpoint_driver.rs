use kirin_interpreter::{AbstractValue, ProductValue, WideningStrategy};
use kirin_ir::{Block, CompileStage, HasBottom, Lattice, ResultValue, SpecializedFunction};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::abstract_interp::state::{AbstractFrame, FuncState, FuncSummary, StagedKey, Worklist};
use crate::control::{Control, CursorExt};
use crate::env::{AbstractEnv, Env};
use crate::error::InterpreterError;
use crate::execute::{Execute, StackEntry};

/// Shared fixpoint driver for abstract interpretation.
///
/// Implementing this trait's required field-accessor methods gives access to
/// the provided `run_fixpoint` method, eliminating duplication between
/// `AbstractInterp` and any wrapping newtype (e.g. `AbstractMultiInterp`).
pub trait FixpointDriver: AbstractEnv + Sized
where
    Self::Value: Clone + AbstractValue,
{
    type Cursor;

    fn summaries_ref(&self) -> &FxHashMap<StagedKey, FuncSummary<Self::Value>>;
    fn summaries_mut(&mut self) -> &mut FxHashMap<StagedKey, FuncSummary<Self::Value>>;
    fn func_states_mut(&mut self) -> &mut FxHashMap<StagedKey, FuncState<Self::Value>>;
    fn func_worklist_mut(&mut self) -> &mut Worklist<StagedKey>;
    fn cursor_stack_ref(&self) -> &[StackEntry<Self::Cursor, Self::Value>];
    fn cursor_stack_mut(&mut self) -> &mut Vec<StackEntry<Self::Cursor, Self::Value>>;
    fn call_graph_mut(&mut self) -> &mut FxHashMap<StagedKey, FxHashSet<AbstractFrame>>;
    fn fn_visit_counts_mut(&mut self) -> &mut FxHashMap<StagedKey, usize>;
    fn widening_strategy(&self) -> WideningStrategy;
    fn make_abstract_cursor(
        &self,
        stage_id: CompileStage,
        block: Block,
        args: Vec<Self::Value>,
    ) -> Self::Cursor;
    fn set_current_key(&mut self, key: Option<StagedKey>);
    fn get_current_key(&self) -> Option<StagedKey>;
    fn entry_block_for(
        &self,
        func: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, Self::Error>;

    // ---------------------------------------------------------------------------
    // Provided: shared fixpoint implementation
    // ---------------------------------------------------------------------------

    fn run_fixpoint(
        &mut self,
        entry_fn: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<Self::Value>,
    ) -> Result<Option<Self::Value>, Self::Error>
    where
        Self::Cursor: Execute<Self>,
        Self: Env<Ext = CursorExt<Self::Cursor>>,
        Self::Value: ProductValue,
    {
        let entry_key = (entry_fn, stage_id);
        let entry_block = self.entry_block_for(entry_fn, stage_id)?;
        self.summaries_mut().insert(
            entry_key,
            FuncSummary {
                input: args,
                output: None,
                entry_block,
            },
        );
        self.func_worklist_mut().push(entry_key);

        while let Some(key) = self.func_worklist_mut().pop() {
            analyze_function(self, key)?;
        }

        Ok(self
            .summaries_ref()
            .get(&entry_key)
            .and_then(|s| s.output.clone()))
    }
}

// ---------------------------------------------------------------------------
// analyze_function
// ---------------------------------------------------------------------------

pub(crate) fn analyze_function<D>(driver: &mut D, key: StagedKey) -> Result<(), D::Error>
where
    D: FixpointDriver,
    D::Value: Clone + AbstractValue + ProductValue,
    D::Cursor: Execute<D>,
    D: Env<Ext = CursorExt<D::Cursor>>,
{
    let (_, func_stage) = key;
    let (entry_block, input) = {
        let s = driver
            .summaries_ref()
            .get(&key)
            .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
        (s.entry_block, s.input.clone())
    };

    let mut state = FuncState::new();
    state.block_in.insert(entry_block, input);
    state.block_worklist.push(entry_block);
    driver.func_states_mut().insert(key, state);
    driver.set_current_key(Some(key));

    loop {
        while !driver.cursor_stack_ref().is_empty() {
            step_cursor(driver, key)?;
        }

        let block = driver
            .func_states_mut()
            .get_mut(&key)
            .and_then(|s| s.block_worklist.pop());
        let Some(block) = block else { break };

        let block_args = driver
            .func_states_mut()
            .get(&key)
            .and_then(|s| s.block_in.get(&block).cloned())
            .unwrap_or_default();

        let cursor = driver.make_abstract_cursor(func_stage, block, block_args);
        driver.cursor_stack_mut().push(StackEntry::new(cursor));

        while !driver.cursor_stack_ref().is_empty() {
            step_cursor(driver, key)?;
        }
    }

    driver.set_current_key(None);
    Ok(())
}

// ---------------------------------------------------------------------------
// step_cursor
// ---------------------------------------------------------------------------

fn step_cursor<D>(driver: &mut D, key: StagedKey) -> Result<(), D::Error>
where
    D: FixpointDriver,
    D::Value: Clone + AbstractValue + ProductValue,
    D::Cursor: Execute<D>,
    D: Env<Ext = CursorExt<D::Cursor>>,
{
    let Some(mut entry) = driver.cursor_stack_mut().pop() else {
        return Ok(());
    };
    let inbox = entry.inbox.take();
    let effect = entry.cursor.execute(driver, inbox)?;

    match effect {
        Control::Advance => {
            driver.cursor_stack_mut().push(entry);
        }
        Control::Ext(CursorExt::Push(new_cursor)) => {
            driver.cursor_stack_mut().push(entry);
            driver.cursor_stack_mut().push(StackEntry::new(new_cursor));
        }
        Control::Ext(CursorExt::Pop) => {}
        Control::Yield(v) => {
            if let Some(parent) = driver.cursor_stack_mut().last_mut() {
                parent.inbox = Some(v);
            }
        }
        Control::Return(v) => {
            driver.cursor_stack_mut().clear();
            record_return(driver, key, v)?;
        }
        Control::Jump(block, args) => {
            driver.enqueue_block(block, args);
        }
        Control::Fork(branches) => {
            for (block, args) in branches {
                driver.enqueue_block(block, args);
            }
        }
        Control::Call {
            callee,
            stage: callee_stage,
            args,
            results,
        } => {
            driver.cursor_stack_mut().push(entry);
            let call_result = handle_call(driver, key, callee, callee_stage, &results, args)?;
            driver.write_results(&results, call_result)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// handle_call
// ---------------------------------------------------------------------------

fn handle_call<D>(
    driver: &mut D,
    caller_key: StagedKey,
    callee: SpecializedFunction,
    callee_stage: CompileStage,
    call_site_results: &[ResultValue],
    new_args: Vec<D::Value>,
) -> Result<D::Value, D::Error>
where
    D: FixpointDriver,
    D::Value: Clone + AbstractValue + ProductValue,
    D::Cursor: Execute<D>,
    D: Env<Ext = CursorExt<D::Cursor>>,
{
    let callee_key = (callee, callee_stage);
    let frame = AbstractFrame {
        func: caller_key.0,
        stage: caller_key.1,
        results: call_site_results.to_vec(),
    };
    driver
        .call_graph_mut()
        .entry(callee_key)
        .or_default()
        .insert(frame);

    if let Some(existing_input) = driver
        .summaries_ref()
        .get(&callee_key)
        .map(|s| s.input.clone())
    {
        if existing_input.len() != new_args.len() {
            return Err(D::Error::from(InterpreterError::ArityMismatch {
                expected: existing_input.len(),
                got: new_args.len(),
            }));
        }

        let widening = driver.widening_strategy();
        let fn_visits = *driver.fn_visit_counts_mut().get(&callee_key).unwrap_or(&0);
        let merged: Vec<D::Value> = existing_input
            .iter()
            .zip(new_args.iter())
            .map(|(e, a)| widening.merge(e, a, fn_visits))
            .collect();
        let input_grew = merged
            .iter()
            .zip(existing_input.iter())
            .any(|(n, o)| !n.is_subseteq(o));

        if input_grew {
            driver.summaries_mut().get_mut(&callee_key).unwrap().input = merged;
            *driver.fn_visit_counts_mut().entry(callee_key).or_insert(0) += 1;
            driver.func_worklist_mut().push(callee_key);
        }

        Ok(driver
            .summaries_ref()
            .get(&callee_key)
            .unwrap()
            .output
            .clone()
            .unwrap_or_else(D::Value::bottom))
    } else {
        let entry_block = driver.entry_block_for(callee, callee_stage)?;
        driver.summaries_mut().insert(
            callee_key,
            FuncSummary {
                input: new_args,
                output: None,
                entry_block,
            },
        );
        driver.func_worklist_mut().push(callee_key);
        Ok(D::Value::bottom())
    }
}

// ---------------------------------------------------------------------------
// record_return
// ---------------------------------------------------------------------------

fn record_return<D>(driver: &mut D, key: StagedKey, v: D::Value) -> Result<(), D::Error>
where
    D: FixpointDriver,
    D::Value: Clone + AbstractValue,
{
    let summary = driver
        .summaries_mut()
        .get_mut(&key)
        .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
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
        let caller_keys: Vec<StagedKey> = driver
            .call_graph_mut()
            .get(&key)
            .into_iter()
            .flatten()
            .map(|f| (f.func, f.stage))
            .collect();
        for ck in caller_keys {
            driver.func_worklist_mut().push(ck);
        }
    }
    Ok(())
}
