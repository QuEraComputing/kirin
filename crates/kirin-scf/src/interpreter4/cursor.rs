use kirin::prelude::{Block, Dialect, ResultValue};
use kirin_interpreter::ProductValue;
use kirin_interpreter_4::concrete::{Action, SingleStage};
use kirin_interpreter_4::cursor::BlockCursor;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::execute::Execute;
use kirin_interpreter_4::lift::Lift;
use kirin_interpreter_4::traits::{Machine, ValueStore};

use crate::ForLoopValue;

// ---------------------------------------------------------------------------
// IfCursor — two-phase inline execution for scf.if
// ---------------------------------------------------------------------------

enum IfPhase<V> {
    /// First execute: push a BlockCursor for the chosen body.
    PushBody {
        body: Block,
        results: Vec<ResultValue>,
    },
    /// Second execute: take pending_yield, write to results, pop.
    CollectYield { results: Vec<ResultValue> },
    /// Sentinel — should never be reached.
    Done(std::marker::PhantomData<V>),
}

pub struct IfCursor<V> {
    phase: IfPhase<V>,
}

impl<V> IfCursor<V> {
    pub fn new(body: Block, results: Vec<ResultValue>) -> Self {
        Self {
            phase: IfPhase::PushBody { body, results },
        }
    }
}

impl<'ir, L, V, M, C> Execute<SingleStage<'ir, L, V, M, C>> for IfCursor<V>
where
    L: Dialect,
    V: Clone + ProductValue,
    M: Machine<Error = InterpreterError>,
    C: Lift<BlockCursor<V>>,
{
    fn execute(
        &mut self,
        interp: &mut SingleStage<'ir, L, V, M, C>,
    ) -> Result<Action<V, M::Effect, C>, InterpreterError> {
        match std::mem::replace(&mut self.phase, IfPhase::Done(std::marker::PhantomData)) {
            IfPhase::PushBody { body, results } => {
                let stage = interp.stage_info();
                let cursor = BlockCursor::new(stage, body, vec![], vec![]);
                self.phase = IfPhase::CollectYield { results };
                Ok(Action::Push(Lift::lift(cursor)))
            }
            IfPhase::CollectYield { results } => {
                if let Some(product) = interp.take_pending_yield() {
                    write_product(interp, &results, product)?;
                }
                Ok(Action::Pop)
            }
            IfPhase::Done(_) => Err(InterpreterError::UnhandledEffect(
                "IfCursor executed after completion".into(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// ForCursor — multi-phase inline execution for scf.for
// ---------------------------------------------------------------------------

enum ForPhase<V> {
    /// Check condition, push body cursor if true.
    CheckAndPush {
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
    },
    /// Collect yield, step iv, go back to CheckAndPush.
    CollectAndStep {
        iv: V,
        end: V,
        step: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
    },
    /// Sentinel.
    Done(std::marker::PhantomData<V>),
}

pub struct ForCursor<V> {
    phase: ForPhase<V>,
}

impl<V> ForCursor<V> {
    pub fn new(
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
    ) -> Self {
        Self {
            phase: ForPhase::CheckAndPush {
                iv,
                end,
                step,
                carried,
                body,
                init_arg_count,
                results,
            },
        }
    }
}

impl<'ir, L, V, M, C> Execute<SingleStage<'ir, L, V, M, C>> for ForCursor<V>
where
    L: Dialect,
    V: Clone + ProductValue + ForLoopValue,
    M: Machine<Error = InterpreterError>,
    C: Lift<BlockCursor<V>>,
{
    fn execute(
        &mut self,
        interp: &mut SingleStage<'ir, L, V, M, C>,
    ) -> Result<Action<V, M::Effect, C>, InterpreterError> {
        match std::mem::replace(&mut self.phase, ForPhase::Done(std::marker::PhantomData)) {
            ForPhase::CheckAndPush {
                iv,
                end,
                step,
                carried,
                body,
                init_arg_count,
                results,
            } => {
                if iv.loop_condition(&end) != Some(true) {
                    // Loop done — write final carried state to results.
                    write_product(interp, &results, carried)?;
                    return Ok(Action::Pop);
                }

                // Build block args: [iv, ...carried]
                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(iv.clone());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried);
                }

                let stage = interp.stage_info();
                let cursor = BlockCursor::new(stage, body, block_args, vec![]);
                self.phase = ForPhase::CollectAndStep {
                    iv,
                    end,
                    step,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(Action::Push(Lift::lift(cursor)))
            }
            ForPhase::CollectAndStep {
                iv,
                end,
                step,
                body,
                init_arg_count,
                results,
            } => {
                let carried = interp.take_pending_yield().ok_or_else(|| {
                    InterpreterError::UnhandledEffect("scf.for body did not yield a value".into())
                })?;

                let next_iv = iv.loop_step(&step).ok_or_else(|| {
                    InterpreterError::UnhandledEffect(
                        "scf.for: induction variable overflow during loop step".into(),
                    )
                })?;

                // Go back to check-and-push with updated state.
                self.phase = ForPhase::CheckAndPush {
                    iv: next_iv,
                    end,
                    step,
                    carried,
                    body,
                    init_arg_count,
                    results,
                };
                // Return Advance so driver pushes us back for next iteration.
                Ok(Action::Advance)
            }
            ForPhase::Done(_) => Err(InterpreterError::UnhandledEffect(
                "ForCursor executed after completion".into(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: write product value to result slots
// ---------------------------------------------------------------------------

fn write_product<I>(
    interp: &mut I,
    results: &[ResultValue],
    product: I::Value,
) -> Result<(), InterpreterError>
where
    I: ValueStore<Error = InterpreterError>,
    I::Value: ProductValue,
{
    if results.is_empty() {
        return Ok(());
    }
    if results.len() == 1 {
        interp.write(results[0], product)?;
    } else if let Some(components) = product.as_product() {
        for (result, value) in results.iter().zip(components.iter()) {
            interp.write(*result, value.clone())?;
        }
    } else {
        interp.write(results[0], product)?;
    }
    Ok(())
}
