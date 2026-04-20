use kirin_interpreter::{AbstractValue, ProductValue};
use kirin_ir::{CompileStage, Dialect, HasStageInfo, ResultValue, SpecializedFunction, StageMeta};

use crate::abstract_call_dispatch::AbstractCallDispatch;
use crate::control::{Control, CursorExt};
use crate::env::{AbstractEnv, Env};
use crate::error::InterpreterError;
use crate::execute::{Execute, StackEntry};

use super::{
    AbstractInterp,
    state::{AbstractFrame, FuncState, FuncSummary, StagedKey},
};

pub(super) fn run<'ir, S, L, V, C>(
    interp: &mut AbstractInterp<'ir, S, L, V, C>,
    entry_fn: SpecializedFunction,
    stage_id: CompileStage,
    args: Vec<V>,
) -> Result<Option<V>, InterpreterError>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    C: Execute<AbstractInterp<'ir, S, L, V, C>>,
{
    let entry_block = S::entry_block_for(interp.handle.pipeline, entry_fn, stage_id)?;
    let entry_key = (entry_fn, stage_id);
    interp.summaries.insert(
        entry_key,
        FuncSummary {
            input: args,
            output: None,
            entry_block,
        },
    );
    interp.func_worklist.push(entry_key);

    while let Some(key) = interp.func_worklist.pop() {
        analyze_function(interp, key)?;
    }

    Ok(interp
        .summaries
        .get(&entry_key)
        .and_then(|s| s.output.clone()))
}

fn analyze_function<'ir, S, L, V, C>(
    interp: &mut AbstractInterp<'ir, S, L, V, C>,
    key: StagedKey,
) -> Result<(), InterpreterError>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    C: Execute<AbstractInterp<'ir, S, L, V, C>>,
{
    let (_, func_stage) = key;
    let (entry_block, input) = {
        let s = interp
            .summaries
            .get(&key)
            .ok_or(InterpreterError::MissingEntry)?;
        (s.entry_block, s.input.clone())
    };

    let mut state = FuncState::new();
    state.block_in.insert(entry_block, input);
    state.block_worklist.push(entry_block);
    interp.func_states.insert(key, state);
    interp.current_key = Some(key);

    loop {
        while !interp.cursor_stack.is_empty() {
            step_cursor(interp, key)?;
        }

        let block = {
            let state = interp.func_states.get_mut(&key).unwrap();
            state.block_worklist.pop()
        };
        let Some(block) = block else { break };

        let block_args = interp
            .func_states
            .get(&key)
            .and_then(|s| s.block_in.get(&block).cloned())
            .unwrap_or_default();

        let cursor = S::make_abstract_cursor(interp.handle.pipeline, func_stage, block, block_args);
        interp.cursor_stack.push(StackEntry::new(cursor));

        while !interp.cursor_stack.is_empty() {
            step_cursor(interp, key)?;
        }
    }

    interp.current_key = None;
    Ok(())
}

fn step_cursor<'ir, S, L, V, C>(
    interp: &mut AbstractInterp<'ir, S, L, V, C>,
    key: StagedKey,
) -> Result<(), InterpreterError>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    C: Execute<AbstractInterp<'ir, S, L, V, C>>,
{
    let Some(mut entry) = interp.cursor_stack.pop() else {
        return Ok(());
    };

    let inbox = entry.inbox.take();
    let effect: Control<V, CursorExt<C>> = entry.cursor.execute(interp, inbox)?;

    match effect {
        Control::Advance => {
            interp.cursor_stack.push(entry);
        }
        Control::Ext(CursorExt::Push(new_cursor)) => {
            interp.cursor_stack.push(entry);
            interp.cursor_stack.push(StackEntry::new(new_cursor));
        }
        Control::Ext(CursorExt::Pop) => {}
        Control::Yield(v) => {
            if let Some(parent) = interp.cursor_stack.last_mut() {
                parent.inbox = Some(v);
            }
        }
        Control::Return(v) => {
            interp.cursor_stack.clear();
            interp.record_return_inner(key, v)?;
        }
        Control::Jump(block, args) => {
            interp.enqueue_block(block, args);
        }
        Control::Fork(branches) => {
            for (block, args) in branches {
                interp.enqueue_block(block, args);
            }
        }
        Control::Call {
            callee,
            stage: callee_stage,
            args,
            results,
        } => {
            interp.cursor_stack.push(entry);
            let call_result = handle_call(interp, key, callee, callee_stage, &results, args)?;
            interp.write_results(&results, call_result)?;
        }
    }

    Ok(())
}

fn handle_call<'ir, S, L, V, C>(
    interp: &mut AbstractInterp<'ir, S, L, V, C>,
    caller_key: StagedKey,
    callee: SpecializedFunction,
    callee_stage: CompileStage,
    call_site_results: &[ResultValue],
    new_args: Vec<V>,
) -> Result<V, InterpreterError>
where
    S: StageMeta + HasStageInfo<L> + AbstractCallDispatch<V, C>,
    L: Dialect,
    V: Clone + AbstractValue + ProductValue,
    C: Execute<AbstractInterp<'ir, S, L, V, C>>,
{
    let callee_key = (callee, callee_stage);

    let frame = AbstractFrame {
        func: caller_key.0,
        stage: caller_key.1,
        results: call_site_results.to_vec(),
    };
    interp
        .call_graph
        .entry(callee_key)
        .or_default()
        .insert(frame);

    if let Some(summary) = interp.summaries.get(&callee_key) {
        let existing_input = summary.input.clone();

        if existing_input.len() != new_args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: existing_input.len(),
                got: new_args.len(),
            });
        }

        let widening = interp.widening;
        let fn_visits = *interp.fn_visit_counts.get(&callee_key).unwrap_or(&0);
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
            interp.summaries.get_mut(&callee_key).unwrap().input = merged;
            *interp.fn_visit_counts.entry(callee_key).or_insert(0) += 1;
            interp.func_worklist.push(callee_key);
        }

        Ok(interp
            .summaries
            .get(&callee_key)
            .unwrap()
            .output
            .clone()
            .unwrap_or_else(V::bottom))
    } else {
        let entry_block = S::entry_block_for(interp.handle.pipeline, callee, callee_stage)?;
        interp.summaries.insert(
            callee_key,
            FuncSummary {
                input: new_args,
                output: None,
                entry_block,
            },
        );
        interp.func_worklist.push(callee_key);
        Ok(V::bottom())
    }
}
